#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use clap::{ArgAction, Args, Parser, Subcommand, builder::BoolishValueParser, builder::styling};
use esdiag::cli_output::{
    BundleResult, CliFailure, CliFailureCategory, CliOutcome, CompletedStages, DiagnosticResult, FileCounts,
    IncludedDiagnosticResult, JobInputResult, JobProcessResult, JobSaveResult, JobSendResult, JobStage,
    KeystoreOperation, OutputFormat, ProcessResult, SavedJobResult, SendResult, write_terminal_outcome,
};
use esdiag::job::{FailedStage, JobExecutionFailure, JobOutcome, SavedJobNotFound};
#[cfg(feature = "server")]
use esdiag::server::{AuthProvider, RuntimeMode, Server, ServerStartOptions};
#[cfg(feature = "setup")]
use esdiag::setup;
use esdiag::{
    client::Client,
    data::{
        Application, HostRole, KnownHost, KnownHostBuilder, KnownHostCliUpdate, OutputDeployment, SecretAuth, Settings,
        Uri, add_secret, clear_unlock_lease, collect_application, create_keystore, default_unlock_ttl,
        get_keystore_path, get_password_for_secret_commands, get_unlock_status, keystore_exists, parse_unlock_ttl,
        remove_secret, resolve_secret_auth, rotate_keystore_password, update_secret, upsert_secret_auth,
        validate_existing_keystore_password, write_unlock_lease,
    },
    env::LOG_LEVEL,
    exporter::Exporter,
    onboarding::{
        CollectHostInput, OutputDeploymentInput, inspect as inspect_onboarding, save_collect_host, save_default_job,
        save_default_processing_job, save_output_deployment, save_user,
    },
    processor::{CollectionResult, DiagnosticOutcome, Identifiers, default_collect_archive_name},
    receiver::{
        ElasticCloudAdminRequestError, ElasticsearchRequestError, KibanaRequestError, LogstashRequestError, Receiver,
    },
    uploader,
};
use eyre::{Result, eyre};
use redact::Secret;
use std::{
    io::{IsTerminal, Write},
    path::PathBuf,
    process::{Command, ExitCode},
    str::FromStr,
    time::Duration,
};
use tracing_subscriber::{EnvFilter, fmt};
use url::Url;

// CLI Styling
const STYLES: styling::Styles = styling::Styles::styled()
    .header(styling::AnsiColor::BrightWhite.on_default())
    .usage(styling::AnsiColor::BrightWhite.on_default())
    .literal(styling::AnsiColor::Green.on_default())
    .placeholder(styling::AnsiColor::Cyan.on_default());

// Define command line arguments
#[derive(Debug, Parser)]
#[command(name = "esdiag", version, styles = STYLES)]
#[command(about = "Elastic Stack Diagnostics (esdiag) - collect diagnostics and import into Elasticsearch", long_about = None)]
struct Cli {
    /// Enable debug logging
    #[arg(global = true, long)]
    debug: bool,
    /// Enable agent-oriented low-noise CLI behavior
    #[arg(global = true, long, short = 'a')]
    agent: bool,
    /// Result representation for finite command outcomes
    #[arg(global = true, long, value_enum, default_value_t = OutputFormat::Yaml)]
    format: OutputFormat,
    /// Commands
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Collect a diagnostic bundle from a known host's API endpoints, writes output to a directory
    Collect {
        /// The host to collect diagnostics from
        #[arg(help = "The Elastic Stack host to collect diagnostics from")]
        host: String,
        /// The output directory to save the diagnostics to
        #[arg(help = "An existing directory to create a diagnostic directory and files in")]
        output: String,
        /// Diagnostic type
        #[arg(
            long,
            default_value = "standard",
            help = "Diagnostic type (minimal, light, standard, support)"
        )]
        r#type: String,
        /// Explicitly include APIs
        #[arg(long, help = "Comma-separated list of APIs to include", value_delimiter = ',')]
        include: Option<Vec<String>>,
        /// Explicitly exclude APIs
        #[arg(long, help = "Comma-separated list of APIs to exclude", value_delimiter = ',')]
        exclude: Option<Vec<String>>,
        /// Override the embedded sources.yml for the detected Elasticsearch or Logstash job.
        /// The file must match the active product or the command fails before collection.
        #[arg(long)]
        sources: Option<String>,
        /// Diagnostic report account name
        #[arg(help = "Diagnostic report account name", long)]
        account: Option<String>,
        /// Case number added to diagnostic report
        #[arg(help = "Diagnostic report case number", long, short)]
        case: Option<String>,
        /// Diagnostic report opportunity
        #[arg(help = "Diagnostic report opportunity", long, short)]
        opportunity: Option<String>,
        /// Diagnostic report user
        #[arg(help = "Diagnostic report user", long = "user", short, value_name = "USER")]
        user: Option<String>,
        /// Elastic Upload Service upload id or URL for immediate upload after collection
        #[arg(
            help = "Elastic Upload Service upload id or URL for immediate upload after collection",
            long = "upload"
        )]
        upload_id: Option<String>,
        /// Save the effective invocation as a named job before continuing execution
        #[cfg(feature = "keystore")]
        #[arg(long = "save-job", value_name = "NAME")]
        save_job: Option<String>,
    },
    /// Start a web server to receive diagnostic bundle uploads
    #[cfg(feature = "server")]
    Serve {
        /// The port to bind the server to
        #[arg(help = "The port to bind the server to", long, short, default_value = "2501")]
        port: u16,
        /// Target to send processed diagnostic documents to
        #[arg(
            long_help = "Target to send the processed diagnostic documents to (known host, file, stdout, or env). Strings will be checked against the known hosts stored in `~/.esdiag/hosts.yml` and will fallback to a filename if not found. Use `-` for stdout. If nothing is provided, the output will try using the environment variables: ESDIAG_OUTPUT_URL, ESDIAG_OUTPUT_APIKEY, ESDIAG_OUTPUT_USERNAME, and ESDIAG_OUTPUT_PASSWORD."
        )]
        output: Option<String>,
        /// Web runtime mode for the server
        #[arg(long, value_enum, help = "Web runtime mode: user or service")]
        mode: Option<RuntimeMode>,
        /// Request authentication provider for the server
        #[arg(long, value_enum, help = "Request authentication provider: google-iap or none")]
        auth_provider: Option<AuthProvider>,
        /// Optional comma-separated web feature allowlist (advanced, job-builder)
        #[arg(long, value_name = "FEATURES")]
        web_features: Option<String>,
        /// Kibana URL to display in the web interface
        #[arg(
            long,
            long_help = "Kibana URL to display in the web interface. If not provided, will use the ESDIAG_KIBANA_URL environment variable."
        )]
        kibana: Option<String>,
    },
    /// Manage saved host connections in `~/.esdiag/hosts.yml`
    Host {
        #[command(subcommand)]
        command: HostCommands,
    },
    /// Interactively configure a repeatable local diagnostic workflow
    Init,
    /// Manage encrypted secrets in the local keystore
    #[command(alias = "secret")]
    Keystore {
        #[command(subcommand)]
        command: KeystoreCommands,
    },
    /// Receives a diagnostic from the input, processes it, and sends processed docs to the output
    Process {
        /// Source to read diagnostic data from
        #[arg(help = "Source to read diagnostic data from (archive, directory, known host or Elastic uploader URL)")]
        input: String,

        /// Target to send processed diagnostic documents to
        #[arg(
            long_help = "Target to send the processed diagnostic documents to (known host, file, stdout, or env). Strings will be checked against the known hosts stored in `~/.esdiag/hosts.yml` and will fallback to a filename if not found. Use `-` for stdout. If nothing is provided, the output will try using the environment variables: ESDIAG_OUTPUT_URL, ESDIAG_OUTPUT_APIKEY, ESDIAG_OUTPUT_USERNAME, and ESDIAG_OUTPUT_PASSWORD."
        )]
        output: Option<String>,

        /// Diagnostic report account name
        #[arg(help = "Diagnostic report account name", long)]
        account: Option<String>,

        /// Case number added to diagnostic report
        #[arg(help = "Diagnostic report case number", long, short)]
        case: Option<String>,

        /// Diagnostic report opportunity
        #[arg(help = "Diagnostic report opportunity", long, short)]
        opportunity: Option<String>,

        /// Diagnostic report user
        #[arg(help = "Diagnostic report user", long, short)]
        user: Option<String>,
        /// Override the embedded sources.yml for the detected Elasticsearch or Logstash job.
        /// The file must match the active product or the command fails before processing.
        #[arg(long)]
        sources: Option<String>,
        /// Save the effective invocation as a named job before continuing execution
        #[cfg(feature = "keystore")]
        #[arg(long = "save-job", value_name = "NAME")]
        save_job: Option<String>,
    },
    /// Upload a raw diagnostic archive to Elastic Upload Service
    Upload {
        /// Local diagnostic archive to upload
        #[arg(help = "Local diagnostic archive file path")]
        file_name: String,
        /// Elastic Upload Service upload id or URL
        #[arg(help = "Upload id or Elastic Upload Service URL")]
        upload_id: String,
        /// Upload API base URL
        #[arg(
            long,
            default_value = uploader::DEFAULT_UPLOAD_API_URL,
            help = "Elastic Upload Service base URL"
        )]
        api_url: String,
    },
    #[cfg(feature = "setup")]
    /// Import assets (templates, ingest pipelines, etc.) to a known Elasticsearch host
    Setup {
        /// Known Elasticsearch host to import assets into; if omitted the ESDIAG_OUTPUT_URL, ESDIAG_OUTPUT_APIKEY, ESDIAG_OUTPUT_USERNAME, ESDIAG_OUTPUT_PASSWORD variables will be checked.
        #[arg(
            help = "Known Elasticsearch host to import assets into; if omitted the ESDIAG_OUTPUT_URL, ESDIAG_OUTPUT_APIKEY, ESDIAG_OUTPUT_USERNAME, ESDIAG_OUTPUT_PASSWORD variables will be checked."
        )]
        host: Option<String>,
    },
    /// Manage saved diagnostic jobs
    #[cfg(feature = "keystore")]
    Job {
        #[command(subcommand)]
        command: JobCommands,
    },
}

#[cfg(feature = "keystore")]
#[derive(Debug, Subcommand)]
enum JobCommands {
    /// Run a saved job by name
    Run {
        /// Name of the saved job to run
        name: String,
    },
    /// List all saved jobs
    List,
    /// Delete a saved job by name
    Delete {
        /// Name of the saved job to delete
        name: String,
    },
}

#[derive(Debug, Subcommand)]
enum HostCommands {
    /// Add a saved host
    Add {
        /// A name to identify this host
        #[arg(help = "A name to identify this host")]
        name: String,
        /// A concrete URL or resolved template reference
        #[arg(help = "A concrete URL, a template definition target, or a resolved template reference")]
        target: String,
        /// Application of this host when the target cannot infer it
        #[arg(help = "Application of this host when the target cannot infer it", long)]
        app: Option<Application>,
        /// Treat <target> as a saved host URL template instead of a concrete URL
        #[arg(help = "Persist <target> as a saved host URL template", long, action = ArgAction::SetTrue)]
        url_template: bool,
        #[command(flatten)]
        args: HostMutationArgs,
    },
    /// Update an existing saved host
    Update {
        /// Name of the saved host to update
        name: String,
        #[command(flatten)]
        args: HostMutationArgs,
    },
    /// Remove an existing saved host
    Remove {
        /// Name of the saved host to remove
        name: String,
    },
    /// List all saved hosts
    List,
    /// Test authentication for a saved host
    Auth {
        /// Saved host name or resolved template reference to test
        target: String,
    },
    #[command(external_subcommand)]
    Legacy(Vec<String>),
}

#[derive(Debug, Args, Clone)]
struct HostMutationArgs {
    /// Accept invalid certificates
    #[arg(
        help = "Accept invalid certificates",
        long,
        value_parser = BoolishValueParser::new()
    )]
    accept_invalid_certs: Option<bool>,
    /// ApiKey for authentication
    #[arg(
        help = "ApiKey, passed as http header",
        long,
        short = 'k',
        conflicts_with_all = &["username", "password"]
    )]
    apikey: Option<String>,
    /// Username for authentication
    #[arg(
        help = "Username for authentication",
        long = "user",
        visible_alias = "username",
        short
    )]
    username: Option<String>,
    /// Password for authentication
    #[arg(help = "Password for authentication", long, short)]
    password: Option<String>,
    /// Secret identifier in the encrypted keystore
    #[arg(
        help = "Secret identifier in the encrypted keystore",
        long,
        conflicts_with_all = &["apikey", "username", "password"]
    )]
    secret: Option<String>,
    /// Comma-separated host roles (collect,send,view)
    #[arg(help = "Comma-separated host roles", long, value_delimiter = ',')]
    roles: Option<Vec<HostRole>>,
}

impl From<HostMutationArgs> for KnownHostCliUpdate {
    fn from(value: HostMutationArgs) -> Self {
        Self {
            accept_invalid_certs: value.accept_invalid_certs,
            apikey: value.apikey.map(Secret::new),
            password: value.password.map(Secret::new),
            roles: value.roles,
            secret: value.secret,
            username: value.username,
        }
    }
}

#[derive(Debug, Subcommand)]
enum KeystoreCommands {
    /// Add a secret to the encrypted keystore
    Add {
        /// Secret identifier
        secret_id: String,
        /// Username for authentication
        #[arg(
            help = "Username for authentication",
            long = "user",
            visible_alias = "username",
            short
        )]
        username: Option<String>,
        /// Password for authentication
        #[arg(
            help = "Password for authentication (prompts when omitted in interactive shells)",
            long,
            short,
            num_args = 0..=1,
            default_missing_value = ""
        )]
        password: Option<String>,
        /// ApiKey for authentication
        #[arg(
            help = "ApiKey, passed as http header (prompts when omitted in interactive shells)",
            long,
            short = 'k',
            num_args = 0..=1,
            default_missing_value = "",
            conflicts_with_all = &["username", "password"]
        )]
        apikey: Option<String>,
    },
    /// Update an existing secret in the encrypted keystore
    Update {
        /// Secret identifier
        secret_id: String,
        /// Username for authentication
        #[arg(
            help = "Username for authentication",
            long = "user",
            visible_alias = "username",
            short
        )]
        username: Option<String>,
        /// Password for authentication
        #[arg(
            help = "Password for authentication (prompts when omitted in interactive shells)",
            long,
            short,
            num_args = 0..=1,
            default_missing_value = ""
        )]
        password: Option<String>,
        /// ApiKey for authentication
        #[arg(
            help = "ApiKey, passed as http header (prompts when omitted in interactive shells)",
            long,
            short = 'k',
            num_args = 0..=1,
            default_missing_value = "",
            conflicts_with_all = &["username", "password"]
        )]
        apikey: Option<String>,
    },
    /// Remove a secret from the encrypted keystore
    Remove {
        /// Secret identifier
        secret_id: String,
        /// Username for authentication
        #[arg(
            help = "Username for authentication",
            long = "user",
            visible_alias = "username",
            short
        )]
        username: Option<String>,
        /// Password for authentication
        #[arg(help = "Password for authentication", long, short)]
        password: Option<String>,
        /// ApiKey for authentication
        #[arg(
            help = "ApiKey, passed as http header",
            long,
            short = 'k',
            conflicts_with_all = &["username", "password"]
        )]
        apikey: Option<String>,
    },
    /// Unlock the local keystore for future CLI runs
    Unlock {
        /// Unlock duration like 90m, 24h, or 7d
        #[arg(long, help = "Unlock duration like 90m, 24h, or 7d")]
        ttl: Option<String>,
    },
    /// Lock the local keystore for future CLI runs
    Lock,
    /// Show local keystore and unlock status
    Status,
    /// Change the keystore password
    Password,
    /// Migrate legacy host credentials in hosts.yml into the keystore
    Migrate,
}

const TOKIO_THREAD_STACK_SIZE: usize = 8 * 1024 * 1024;

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(TOKIO_THREAD_STACK_SIZE)
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("Failed to create runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(async_main()) {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("{error:?}");
            ExitCode::FAILURE
        }
    }
}

async fn async_main() -> Result<ExitCode> {
    // Parse CLI early to configure execution mode and logging.
    let cli = Cli::parse();
    let filter = resolve_tracing_filter(&cli);
    init_tracing(filter);
    let stdout_owned = command_owns_stdout(&cli);
    let no_command = cli.command.is_none();

    std::panic::set_hook(Box::new(|panic| {
        // Log any panics as errors
        tracing::debug!("{:?}", panic);
        tracing::error!("{}", panic);
    }));

    clear_last_run_files()?;

    let format = cli.format;
    match run(cli, format).await {
        Ok(result) => {
            if let Some(outcome) = result.outcome {
                write_terminal_outcome(format, &outcome)?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            tracing::error!("{}", e);
            if !stdout_owned && !no_command {
                let failure = structured_failure(&e);
                write_terminal_outcome(format, &failure)?;
                return Ok(ExitCode::FAILURE);
            }
            Err(eyre!(e))
        }
    }
}

fn init_tracing(filter: EnvFilter) {
    // Bridge `log` records from dependencies when available, but tolerate hosts that
    // already installed a global logger before invoking this binary.
    if let Err(err) = tracing_log::LogTracer::init() {
        eprintln!("tracing log bridge already initialized: {err}");
    }

    let subscriber = fmt().with_env_filter(filter).with_writer(std::io::stderr).finish();
    if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("tracing subscriber already initialized: {err}");
    }
}

#[derive(Debug)]
struct CommandResult {
    outcome: Option<CliOutcome>,
}

impl CommandResult {
    fn outcome(outcome: CliOutcome) -> Self {
        Self { outcome: Some(outcome) }
    }

    fn stream() -> Self {
        Self { outcome: None }
    }
}

fn is_agent_mode(cli: &Cli) -> bool {
    cli.agent || std::env::var_os("CLAUDECODE").is_some()
}

fn command_owns_stdout(cli: &Cli) -> bool {
    let direct_process_stream = matches!(
        &cli.command,
        Some(Commands::Process {
            output: Some(output),
            ..
        }) if output == "-"
    );
    #[cfg(feature = "keystore")]
    let saved_job_stream = matches!(
        &cli.command,
        Some(Commands::Job {
            command: JobCommands::Run { name }
        }) if esdiag::data::load_saved_jobs()
            .ok()
            .and_then(|jobs| jobs.get(name).cloned())
            .and_then(|job| {
                job.process()
                    .map(|process| process.export == esdiag::job::model::ExportTarget::Stdout)
            })
            .unwrap_or(false)
    );
    #[cfg(not(feature = "keystore"))]
    let saved_job_stream = false;
    direct_process_stream || saved_job_stream
}

fn resolve_tracing_filter(cli: &Cli) -> EnvFilter {
    if cli.debug {
        EnvFilter::new("debug")
    } else if is_agent_mode(cli) {
        EnvFilter::new("warn")
    } else {
        EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| EnvFilter::new(LOG_LEVEL))
    }
}

fn classify_failure(error: &eyre::Report) -> CliFailureCategory {
    if let Some(response) = http_response_details(error) {
        return match response.status {
            401 | 403 => CliFailureCategory::AuthenticationFailed,
            404 => CliFailureCategory::NotFound,
            500..=599 => CliFailureCategory::Internal,
            _ => CliFailureCategory::InvalidInput,
        };
    }
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("not found") {
        CliFailureCategory::NotFound
    } else if message.contains("auth") || message.contains("credential") {
        CliFailureCategory::AuthenticationFailed
    } else if message.contains("collect") {
        CliFailureCategory::CollectionFailed
    } else if message.contains("process") {
        CliFailureCategory::ProcessingFailed
    } else if message.contains("upload") || message.contains("send") {
        CliFailureCategory::SendFailed
    } else if message.contains("keystore") || message.contains("secret") {
        CliFailureCategory::KeystoreFailed
    } else {
        CliFailureCategory::InvalidInput
    }
}

fn safe_failure_message(error: &eyre::Report) -> String {
    if let Some(response) = http_response_details(error) {
        return match response.status {
            401 => "The server rejected the request because authentication credentials are required.".to_string(),
            403 => "The server rejected the request because the credentials are not authorized.".to_string(),
            404 => "The requested server resource was not found.".to_string(),
            500..=599 => format!(
                "The server failed to complete the request with HTTP {}.",
                response.status
            ),
            status => format!("The server rejected the request with HTTP {status}."),
        };
    }
    match classify_failure(error) {
        CliFailureCategory::NotFound => "requested resource was not found",
        CliFailureCategory::AuthenticationFailed => "authentication failed",
        CliFailureCategory::CollectionFailed => "diagnostic collection failed",
        CliFailureCategory::ProcessingFailed => "diagnostic processing failed",
        CliFailureCategory::SendFailed => "diagnostic upload failed",
        CliFailureCategory::KeystoreFailed => "keystore operation failed",
        CliFailureCategory::SetupFailed => "setup failed",
        CliFailureCategory::Internal | CliFailureCategory::InvalidInput => "command execution failed",
    }
    .to_string()
}

struct HttpResponseDetails {
    status: u16,
    error_type: Option<String>,
    reason: Option<String>,
}

fn parse_http_response_details(status: u16, body: &str) -> HttpResponseDetails {
    let response_error = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .map(|body| body.get("error").unwrap_or(&body).clone());
    let error_type = response_error
        .as_ref()
        .and_then(|error| error.get("type"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let reason = response_error
        .as_ref()
        .and_then(|error| error.get("reason"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);

    HttpResponseDetails {
        status,
        error_type,
        reason,
    }
}

fn http_response_details(error: &eyre::Report) -> Option<HttpResponseDetails> {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<ElasticsearchRequestError>() {
            return Some(parse_http_response_details(error.status.as_u16(), &error.body));
        }
        if let Some(error) = cause.downcast_ref::<KibanaRequestError>() {
            return Some(parse_http_response_details(error.status.as_u16(), &error.body));
        }
        if let Some(error) = cause.downcast_ref::<LogstashRequestError>() {
            return Some(parse_http_response_details(error.status.as_u16(), &error.body));
        }
        if let Some(error) = cause.downcast_ref::<ElasticCloudAdminRequestError>() {
            return Some(parse_http_response_details(error.status.as_u16(), &error.body));
        }
    }
    None
}

fn structured_failure(error: &eyre::Report) -> CliFailure {
    if let Some(missing_job) = error.downcast_ref::<SavedJobNotFound>() {
        return CliFailure::new(CliFailureCategory::NotFound, "saved job was not found")
            .resource(missing_job.name.clone());
    }
    let Some(job_failure) = error.downcast_ref::<JobExecutionFailure>() else {
        let mut failure = CliFailure::new(classify_failure(error), safe_failure_message(error));
        if let Some(response) = http_response_details(error) {
            failure = failure.http_status(response.status);
            if let Some(error_type) = response.error_type {
                failure = failure.type_(error_type);
            }
            if let Some(reason) = response.reason {
                failure = failure.reason(reason);
            }
        }
        return failure;
    };
    let (category, failed_stage) = match job_failure.stage {
        FailedStage::Collect => (CliFailureCategory::CollectionFailed, JobStage::Collect),
        FailedStage::Process => (CliFailureCategory::ProcessingFailed, JobStage::Process),
        FailedStage::Send => (CliFailureCategory::SendFailed, JobStage::Send),
    };
    let outcome = &job_failure.outcome;
    let completed = CompletedStages {
        save: outcome
            .bundle_path
            .as_ref()
            .filter(|_| outcome.bundle_retained && outcome.bundle_created)
            .map(|path| BundleResult {
                path: path.display().to_string(),
            }),
        process: outcome.execution.as_ref().and_then(execution_process_result),
        send: outcome.upload_slug.as_ref().map(|slug| SendResult {
            destination: format!("https://upload.elastic.co/g/{slug}"),
        }),
    };
    let mut failure = CliFailure::new(category, safe_failure_message(error));
    failure.failed_stage = Some(failed_stage);
    failure.retry_safe = Some(false);
    if completed.save.is_some() || completed.process.is_some() || completed.send.is_some() {
        failure.completed = Some(completed);
    }
    failure
}

fn collection_outcome(result: CollectionResult, upload_destination: Option<String>) -> CliOutcome {
    CliOutcome::ArchiveCollected {
        path: result.path,
        files: FileCounts {
            successful: result.success,
            total: result.total,
        },
        duration_ms: Some(result.duration_ms),
        upload_destination,
    }
}

fn format_execution_failure(outcome: &esdiag::job::outcome::ExecutionOutcome) -> String {
    let failures = outcome
        .stages
        .iter()
        .filter_map(|stage| match &stage.status {
            esdiag::job::outcome::StageStatus::Failed(error) => Some(format!("{:?} failed: {error}", stage.stage)),
            esdiag::job::outcome::StageStatus::Blocked(reason) => Some(format!("{:?} blocked: {reason}", stage.stage)),
            esdiag::job::outcome::StageStatus::Succeeded | esdiag::job::outcome::StageStatus::Skipped(_) => None,
        })
        .collect::<Vec<_>>();
    if failures.is_empty() {
        "Job did not complete successfully".to_string()
    } else {
        failures.join("\n")
    }
}

fn execution_process_result(outcome: &esdiag::job::outcome::ExecutionOutcome) -> Option<ProcessResult> {
    let report = outcome.report.as_ref()?;
    let included = outcome
        .children
        .iter()
        .map(|child| match (child.diagnostic_outcome, child.report()) {
            (DiagnosticOutcome::Skipped(_), _) => IncludedDiagnosticResult::Skipped {
                source: child.path.clone(),
                product: Some(esdiag::processor::display_label(child.application(), child.platform())),
                reason: child
                    .execution_error()
                    .unwrap_or("diagnostic processing skipped")
                    .to_string(),
            },
            (_, Some(report)) => IncludedDiagnosticResult::Completed {
                source: child.path.clone(),
                diagnostic: DiagnosticResult {
                    id: report.diagnostic.metadata.id.clone(),
                    product: report.diagnostic.display_label(),
                    documents: report.diagnostic.docs.created,
                    duration_ms: child.runtime.unwrap_or_default(),
                    source: child.path.clone(),
                    output: String::new(),
                    kibana_url: report.diagnostic.kibana_link.clone(),
                },
            },
            (_, None) => IncludedDiagnosticResult::Failed {
                source: child.path.clone(),
                error: child
                    .execution_error()
                    .unwrap_or("included diagnostic processing failed")
                    .to_string(),
            },
        })
        .collect();

    Some(ProcessResult {
        diagnostic: DiagnosticResult {
            id: report.diagnostic.metadata.id.clone(),
            product: report.diagnostic.display_label(),
            documents: report.diagnostic.docs.created,
            duration_ms: report.diagnostic.processing_duration,
            source: "primary".to_string(),
            output: String::new(),
            kibana_url: report.diagnostic.kibana_link.clone(),
        },
        included,
    })
}

fn job_outcome(name: String, outcome: JobOutcome) -> CliOutcome {
    CliOutcome::JobCompleted {
        job: name,
        save: outcome
            .bundle_path
            .filter(|_| outcome.bundle_retained && outcome.bundle_created)
            .map(|path| BundleResult {
                path: path.display().to_string(),
            }),
        process: outcome.execution.as_ref().and_then(execution_process_result),
        send: outcome.upload_slug.map(|slug| SendResult {
            destination: format!("https://upload.elastic.co/g/{slug}"),
        }),
    }
}

fn host_result(name: String, host: &KnownHost) -> esdiag::cli_output::HostResult {
    esdiag::cli_output::HostResult {
        name,
        app: host.app().map(|app| app.key().to_string()),
        roles: host.roles().iter().map(ToString::to_string).collect(),
        target: host.transport_display(),
        secret_reference: host.secret_reference().map(str::to_string),
    }
}

fn saved_job_result(name: String, job: &esdiag::data::Job) -> SavedJobResult {
    let input = match job.input() {
        esdiag::job::model::Input::Collect {
            host, diagnostic_type, ..
        } => JobInputResult::Collect {
            host: host.clone(),
            diagnostic_type: diagnostic_type.clone(),
        },
        esdiag::job::model::Input::CollectBinding {
            binding,
            diagnostic_type,
            ..
        } => JobInputResult::Collect {
            host: format!("binding:{}", binding.as_str()),
            diagnostic_type: diagnostic_type.clone(),
        },
        esdiag::job::model::Input::Load { uri } => JobInputResult::Load {
            source: uri.to_string(),
        },
        esdiag::job::model::Input::LoadBinding { binding } => JobInputResult::Load {
            source: format!("binding:{}", binding.as_str()),
        },
    };
    let save = job.save().map(|save| JobSaveResult {
        directory: save.dir.as_ref().map(|path| path.display().to_string()),
    });
    let process = job.process().map(|process| JobProcessResult {
        export: match &process.export {
            esdiag::job::model::ExportTarget::KnownHost { name } => format!("host:{name}"),
            esdiag::job::model::ExportTarget::File { path } => path.display().to_string(),
            esdiag::job::model::ExportTarget::Directory { output_dir } => output_dir.display().to_string(),
            esdiag::job::model::ExportTarget::Stdout => "-".to_string(),
            esdiag::job::model::ExportTarget::Binding { binding } => format!("binding:{}", binding.as_str()),
        },
    });
    let send = job.send().map(|send| JobSendResult {
        upload_id: send.upload_id.clone(),
    });
    SavedJobResult {
        name,
        input,
        save,
        process,
        send,
    }
}

#[tracing::instrument(skip_all)]
async fn run(cli: Cli, format: OutputFormat) -> Result<CommandResult> {
    // If there are CLI arguments but no subcommand, avoid starting the desktop/Tauri
    // entrypoint. The desktop UI should only start when launched absolutely without arguments.
    if should_error_for_missing_subcommand(std::env::args_os().len(), cli.command.is_none()) {
        use clap::CommandFactory;
        let mut cmd = Cli::command();
        cmd.print_help()?;
        return Err(eyre!("No subcommand provided. Use --help for usage information."));
    }

    if let Some(command) = cli.command {
        match command {
            #[cfg(feature = "server")]
            Commands::Serve {
                port,
                output,
                mode,
                auth_provider,
                web_features,
                kibana,
            } => {
                tracing::info!("Starting ESDiag server");
                let runtime_mode = resolve_serve_runtime_mode(mode)?;
                let exporter = resolve_serve_exporter(output)?;
                let exporter_owns_stdout = exporter.target_uri() == "stdio://stdout";

                let kibana_url = kibana.unwrap_or_else(|| {
                    esdiag::env::get_string("ESDIAG_KIBANA_URL")
                        .map(|url| esdiag::env::append_kibana_space(&url))
                        .unwrap_or_else(|_| "http://localhost:5601".to_string())
                });

                let (mut server, bound_addr) = Server::start_with_options(
                    [0, 0, 0, 0],
                    port,
                    exporter,
                    kibana_url,
                    runtime_mode,
                    ServerStartOptions {
                        auth_provider,
                        web_features: web_features.as_deref(),
                        ..ServerStartOptions::default()
                    },
                )
                .await?;

                if exporter_owns_stdout {
                    tracing::info!("Server ready at {bound_addr}");
                } else {
                    write_terminal_outcome(
                        format,
                        &CliOutcome::ServerReady {
                            address: bound_addr.ip().to_string(),
                            port: bound_addr.port(),
                            runtime_mode: runtime_mode.to_string(),
                            output: "configured".to_string(),
                        },
                    )?;
                }
                wait_for_shutdown_signal().await?;

                server.shutdown().await;
                Ok(CommandResult::stream())
            }
            Commands::Collect {
                host,
                output,
                r#type,
                include,
                exclude,
                sources,
                account,
                case,
                opportunity,
                user,
                upload_id,
                #[cfg(feature = "keystore")]
                save_job,
            } => {
                #[cfg(feature = "keystore")]
                if let Some(name) = save_job.as_deref() {
                    let identifiers =
                        Identifiers::new(account.clone(), case.clone(), None, opportunity.clone(), user.clone());
                    let job = derive_collect_job(&host, &output, &r#type, upload_id.as_deref(), identifiers)?;
                    esdiag::job::save_job(name, job)?;
                }
                let known_host = Uri::try_from(host)?;
                let output = Uri::try_from(output)?;
                match known_host {
                    Uri::KnownHost(host) | Uri::ElasticCloudAdmin(host) | Uri::ElasticGovCloudAdmin(host) => {
                        ensure_host_role(&host, HostRole::Collect, "collect")?;
                        let application = collect_application(host.app())?;
                        if let Some(sources) = sources {
                            esdiag::processor::init_sources(sources_application_key(application)?, sources)?;
                        }
                        tracing::info!("Collecting diagnostic from {host}");
                        tracing::info!("Saving diagnostic to {output}");
                        let output_dir = match output {
                            Uri::Directory(path) | Uri::File(path) => path,
                            _ => {
                                return Err(eyre!("Collect output must be a local directory path"));
                            }
                        };
                        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
                        let filename = format!("{}.zip", default_collect_archive_name(application, &timestamp));
                        let identifiers = Identifiers::new(account, case, Some(filename), opportunity, user);
                        let binding = esdiag::job::model::BindingKey::try_new("cli-collect-input")?;
                        let mut context = esdiag::job::context::ExecutionContext::default();
                        context
                            .inputs
                            .bind_receiver(binding.clone(), Receiver::try_from(host)?, Some(application));
                        let job = esdiag::job::model::Job::try_new(
                            identifiers,
                            esdiag::job::model::Input::CollectBinding {
                                binding,
                                diagnostic_type: r#type,
                                include,
                                exclude,
                            },
                            Some(esdiag::job::model::SaveTarget::retained(output_dir)),
                            None,
                            upload_id.map(|upload_id| esdiag::job::model::SendTarget { upload_id }),
                        )?;
                        let outcome = esdiag::job::executor::execute_with_context(job, context).await;
                        if !outcome.succeeded() {
                            return Err(eyre!("{}", format_execution_failure(&outcome)));
                        }
                        let result = outcome
                            .collection
                            .ok_or_else(|| eyre!("Collection completed without a collection result"))?;
                        let upload_destination = outcome
                            .upload
                            .as_ref()
                            .map(|upload| format!("https://upload.elastic.co/g/{}", upload.slug));
                        Ok(CommandResult::outcome(collection_outcome(result, upload_destination)))
                    }
                    Uri::ElasticCloud(_) => Err(eyre!("Elastic Cloud API collection not yet implemented")),
                    _ => Err(eyre!("Collect requires a known host")),
                }
            }
            Commands::Init => run_init_wizard().await,
            Commands::Host { command } => match command {
                HostCommands::Add {
                    name,
                    app,
                    target,
                    url_template,
                    args,
                } => {
                    tracing::info!("Adding host {name}");
                    if KnownHost::get_known(&name).is_some() {
                        return Err(eyre!("Host '{name}' already exists"));
                    }
                    let mut update = build_host_cli_update(args);
                    let secret_auth = if update.secret.is_some() {
                        resolve_host_secret_auth(update.secret.as_deref())?
                    } else if url_template {
                        match resolve_same_name_host_secret_auth(&name)? {
                            Some(secret_auth) => {
                                tracing::debug!("Using host name {} as secret_id", name);
                                update.secret = Some(name.clone());
                                Some(secret_auth)
                            }
                            None => None,
                        }
                    } else {
                        None
                    };
                    let host = if url_template {
                        build_host_from_definition(app, &target, true, &update, secret_auth)?
                    } else if let Some(host) = maybe_materialize_template_target(&target)? {
                        host.merge_cli_update(&update, secret_auth)?
                    } else {
                        build_host_from_definition(app, &target, false, &update, secret_auth)?
                    };
                    save_host(&name, host.clone(), "added", !host.is_template()).await?;
                    Ok(CommandResult::outcome(CliOutcome::HostAdded {
                        host: host_result(name, &host),
                    }))
                }
                HostCommands::Update { name, args } => {
                    tracing::info!("Updating host {name}");
                    let update = build_host_cli_update(args);
                    if update.is_empty() {
                        return Err(eyre!(
                            "No host update fields were provided. Use `esdiag host auth {name}` to test the saved host without modifying it."
                        ));
                    }
                    let existing = KnownHost::get_known(&name).ok_or_else(|| eyre!("Host '{name}' not found"))?;
                    let secret_auth = if update.secret.is_some() {
                        resolve_host_secret_auth(update.secret.as_deref())?
                    } else {
                        None
                    };
                    let host = existing.merge_cli_update(&update, secret_auth)?;
                    save_host(&name, host.clone(), "updated", !host.is_template()).await?;
                    Ok(CommandResult::outcome(CliOutcome::HostUpdated {
                        host: host_result(name, &host),
                    }))
                }
                HostCommands::Remove { name } => {
                    tracing::info!("Removing host {name}");
                    let hostfile = delete_host_from_cli(&name)?;
                    tracing::info!("Host {name} successfully deleted from {hostfile}");
                    Ok(CommandResult::outcome(CliOutcome::HostRemoved { name, path: hostfile }))
                }
                HostCommands::List => {
                    let hosts = KnownHost::parse_hosts_yml()?
                        .into_iter()
                        .map(|(name, host)| host_result(name, &host))
                        .collect();
                    Ok(CommandResult::outcome(CliOutcome::HostsListed { hosts }))
                }
                HostCommands::Auth { target } => {
                    tracing::info!("Testing saved host {target}");
                    if let Some(host) = KnownHost::resolve_template_reference(&target)? {
                        validate_host_connection(&target, Uri::try_from(host)?).await?;
                        return Ok(CommandResult::outcome(CliOutcome::HostAuthenticated {
                            name: target,
                            message: None,
                        }));
                    }
                    let host = KnownHost::get_known(&target).ok_or_else(|| eyre!("Host '{target}' not found"))?;
                    if host.is_template() {
                        return Ok(CommandResult::outcome(CliOutcome::HostAuthenticated {
                            name: target.clone(),
                            message: Some(KnownHost::template_guidance(&target)),
                        }));
                    }
                    let uri = Uri::try_from(host)?;
                    validate_host_connection(&target, uri).await?;
                    Ok(CommandResult::outcome(CliOutcome::HostAuthenticated {
                        name: target,
                        message: None,
                    }))
                }
                HostCommands::Legacy(args) => Err(legacy_host_command_error(&args)),
            },
            Commands::Keystore { command } => match command {
                KeystoreCommands::Add {
                    secret_id,
                    username,
                    password,
                    apikey,
                } => {
                    let keystore_password = get_password_for_secret_commands()?;
                    let (username, password, apikey) = resolve_secret_input(username, password, apikey)?;
                    let path = add_secret(&secret_id, username, password, apikey, &keystore_password)?;
                    tracing::info!("Secret '{secret_id}' saved to {path}");
                    Ok(CommandResult::outcome(CliOutcome::KeystoreChanged {
                        operation: KeystoreOperation::Added,
                        secret_id: Some(secret_id),
                        path: Some(path),
                    }))
                }
                KeystoreCommands::Update {
                    secret_id,
                    username,
                    password,
                    apikey,
                } => {
                    let keystore_password = get_password_for_secret_commands()?;
                    let (username, password, apikey) = resolve_secret_input(username, password, apikey)?;
                    let path = update_secret(&secret_id, username, password, apikey, &keystore_password)?;
                    tracing::info!("Secret '{secret_id}' updated in {path}");
                    Ok(CommandResult::outcome(CliOutcome::KeystoreChanged {
                        operation: KeystoreOperation::Updated,
                        secret_id: Some(secret_id),
                        path: Some(path),
                    }))
                }
                KeystoreCommands::Remove {
                    secret_id,
                    username,
                    password,
                    apikey,
                } => {
                    let keystore_password = get_password_for_secret_commands()?;
                    let expected = expected_secret_auth(username, password, apikey)?;
                    let path = remove_secret(&secret_id, expected, &keystore_password)?;
                    tracing::info!("Secret '{secret_id}' deleted from {path}");
                    Ok(CommandResult::outcome(CliOutcome::KeystoreChanged {
                        operation: KeystoreOperation::Removed,
                        secret_id: Some(secret_id),
                        path: Some(path),
                    }))
                }
                KeystoreCommands::Unlock { ttl } => {
                    let ttl = ttl
                        .as_deref()
                        .map(parse_unlock_ttl)
                        .transpose()?
                        .unwrap_or_else(default_unlock_ttl);
                    let unlock_path = unlock_keystore(ttl)?;
                    let status = get_unlock_status()?;
                    if let Some(expires_at_epoch) = status.expires_at_epoch {
                        tracing::info!(
                            "Keystore unlocked via {} until {} ({})",
                            unlock_path.display(),
                            format_epoch(expires_at_epoch),
                            format_remaining_duration(expires_at_epoch)
                        );
                    } else {
                        tracing::info!("Keystore unlocked via {}", unlock_path.display());
                    }
                    Ok(CommandResult::outcome(CliOutcome::KeystoreStatus {
                        exists: status.keystore_exists,
                        unlock_active: status.unlock_active,
                        expires_at_epoch: status.expires_at_epoch,
                    }))
                }
                KeystoreCommands::Lock => {
                    let status = esdiag::data::UnlockStatus {
                        keystore_exists: keystore_exists()?,
                        unlock_active: false,
                        expires_at_epoch: None,
                        unlock_path: esdiag::data::get_unlock_path()?,
                    };
                    if clear_unlock_lease()? {
                        tracing::info!("Keystore unlock lease removed");
                    } else {
                        tracing::info!("Keystore unlock lease was already absent");
                    }
                    Ok(CommandResult::outcome(CliOutcome::KeystoreStatus {
                        exists: status.keystore_exists,
                        unlock_active: false,
                        expires_at_epoch: None,
                    }))
                }
                KeystoreCommands::Status => {
                    let status = get_unlock_status()?;
                    let keystore_path = get_keystore_path()?;
                    tracing::info!(
                        "Keystore: {} ({})",
                        if status.keystore_exists { "present" } else { "absent" },
                        keystore_path.display()
                    );
                    Ok(CommandResult::outcome(CliOutcome::KeystoreStatus {
                        exists: status.keystore_exists,
                        unlock_active: status.unlock_active,
                        expires_at_epoch: status.expires_at_epoch,
                    }))
                }
                KeystoreCommands::Password => {
                    if !keystore_exists()? {
                        return Err(eyre!("No keystore exists to update the password."));
                    }
                    let current_password = get_password_for_secret_commands()?;
                    validate_existing_keystore_password(&current_password)?;
                    let new_password = prompt_new_keystore_password()?;
                    let path = rotate_keystore_password(&current_password, &new_password)?;
                    tracing::info!("Keystore password updated for {path}");
                    Ok(CommandResult::outcome(CliOutcome::KeystoreChanged {
                        operation: KeystoreOperation::PasswordChanged,
                        secret_id: None,
                        path: Some(path),
                    }))
                }
                KeystoreCommands::Migrate => {
                    let keystore_password = get_password_for_secret_commands()?;
                    let (migrated, unchanged) = KnownHost::migrate_hosts_to_keystore(&keystore_password)?;
                    tracing::info!(
                        "Keystore migration complete: migrated {migrated} host(s), unchanged {unchanged} host(s)."
                    );
                    Ok(CommandResult::outcome(CliOutcome::KeystoreChanged {
                        operation: KeystoreOperation::Migrated,
                        secret_id: None,
                        path: Some(format!("migrated={migrated},unchanged={unchanged}")),
                    }))
                }
            },
            Commands::Process {
                input,
                output,
                account,
                case,
                opportunity,
                user,
                sources,
                #[cfg(feature = "keystore")]
                save_job,
            } => {
                let has_explicit_output = output.is_some();
                #[cfg(feature = "keystore")]
                if let Some(name) = save_job.as_deref() {
                    let identifiers =
                        Identifiers::new(account.clone(), case.clone(), None, opportunity.clone(), user.clone());
                    let job = derive_process_job(&input, output.as_deref(), identifiers)?;
                    esdiag::job::save_job(name, job)?;
                }
                let input_uri = Uri::try_from(input)?;
                let output_uri = Uri::try_from(output.clone())?;
                let stdout_owned = matches!(output_uri, Uri::Stream);
                if has_explicit_output {
                    ensure_uri_role(&output_uri, HostRole::Send, "process output")?;
                }

                tracing::info!("input: {}", input_uri);

                let receiver = Receiver::try_from(input_uri.clone())?;
                if let Some(sources) = sources {
                    let application = detect_sources_application_for_process(&input_uri, &receiver).await?;
                    esdiag::processor::init_sources(sources_application_key(application)?, sources)?;
                }
                let identifiers = Identifiers::new(account, case, receiver.filename(), opportunity, user);
                let mut context = esdiag::job::context::ExecutionContext::default();
                let job_input = match &input_uri {
                    Uri::File(_) | Uri::Directory(_) => esdiag::job::model::Input::Load { uri: input_uri.clone() },
                    _ => {
                        let binding = esdiag::job::model::BindingKey::try_new("cli-process-input")?;
                        let application = match &input_uri {
                            Uri::KnownHost(host) | Uri::ElasticCloudAdmin(host) | Uri::ElasticGovCloudAdmin(host) => {
                                host.app()
                            }
                            _ => None,
                        };
                        context.inputs.bind_receiver(binding.clone(), receiver, application);
                        esdiag::job::model::Input::LoadBinding { binding }
                    }
                };
                let export_target = match (&output, &output_uri) {
                    (Some(name), Uri::KnownHost(_)) => {
                        esdiag::job::model::ExportTarget::KnownHost { name: name.clone() }
                    }
                    (_, Uri::File(path)) => esdiag::job::model::ExportTarget::File { path: path.clone() },
                    (_, Uri::Directory(output_dir)) => esdiag::job::model::ExportTarget::Directory {
                        output_dir: output_dir.clone(),
                    },
                    (_, Uri::Stream) => esdiag::job::model::ExportTarget::Stdout,
                    _ => {
                        let binding = esdiag::job::model::BindingKey::try_new("cli-process-output")?;
                        context.bind_document_exporter(
                            binding.clone(),
                            esdiag::exporter::DocumentExporter::try_from(output_uri)?,
                        );
                        esdiag::job::model::ExportTarget::Binding { binding }
                    }
                };
                let job = esdiag::job::model::Job::try_new(
                    identifiers,
                    job_input,
                    None,
                    Some(esdiag::job::model::Process {
                        selection: None,
                        export: export_target,
                    }),
                    None,
                )?;
                let outcome = esdiag::job::executor::execute_with_context(job, context).await;
                let process = execution_process_result(&outcome);
                if !outcome.succeeded() || outcome.diagnostic_outcome() == Some(DiagnosticOutcome::Failed) {
                    return Err(eyre!("{}", format_execution_failure(&outcome)));
                }
                if stdout_owned {
                    Ok(CommandResult::stream())
                } else {
                    let diagnostic = process
                        .map(|result| result.diagnostic)
                        .ok_or_else(|| eyre!("Process completed without a diagnostic report"))?;
                    Ok(CommandResult::outcome(CliOutcome::DiagnosticProcessed { diagnostic }))
                }
            }
            Commands::Upload {
                file_name,
                upload_id,
                api_url,
            } => {
                let file_path = uploader::default_upload_path(&file_name);
                tracing::info!(
                    "Uploading raw diagnostic archive {} to {}",
                    file_path.display(),
                    upload_id
                );
                let response = uploader::upload_file(&file_path, &upload_id, &api_url).await?;
                tracing::info!("Upload complete for slug {}", response.slug);
                Ok(CommandResult::outcome(CliOutcome::DiagnosticUploaded {
                    destination: format!("{}/g/{}", api_url.trim_end_matches('/'), response.slug),
                }))
            }
            #[cfg(feature = "setup")]
            Commands::Setup { host } => {
                if let Some(host) = host {
                    let uri = Uri::try_from(host)?;
                    let client = Client::try_from(uri)?;
                    tracing::info!("Setting up assets in {client}");
                    setup::assets(&client).await?;
                    Ok(CommandResult::outcome(CliOutcome::SetupCompleted {
                        targets: vec![client.to_string()],
                    }))
                } else {
                    tracing::debug!("Setting up assets with the resolved output deployment");
                    let deployment = OutputDeployment::resolve(None, true)?;
                    let es_uri = Uri::try_from(deployment.elasticsearch)?;
                    let es_client = Client::try_from(es_uri)?;
                    tracing::info!("Setting up assets in {es_client}");
                    setup::assets(&es_client).await?;
                    setup::ensure_agent_builder_license(&es_client).await?;
                    let kibana = deployment
                        .kibana
                        .ok_or_else(|| eyre!("Resolved output deployment is missing a Kibana viewer"))?;
                    let kb_uri = Uri::try_from(kibana)?;
                    let kb_client = Client::try_from(kb_uri)?;
                    tracing::info!("Setting up Kibana assets in {kb_client}");
                    setup::assets(&kb_client).await?;
                    Ok(CommandResult::outcome(CliOutcome::SetupCompleted {
                        targets: vec![es_client.to_string(), kb_client.to_string()],
                    }))
                }
            }
            #[cfg(feature = "keystore")]
            Commands::Job { command } => match command {
                JobCommands::List => {
                    let jobs = esdiag::data::load_saved_jobs()?
                        .into_iter()
                        .map(|(name, job)| saved_job_result(name, &job))
                        .collect();
                    Ok(CommandResult::outcome(CliOutcome::JobsListed { jobs }))
                }
                JobCommands::Run { name } => {
                    let stdout_owned = esdiag::data::load_saved_jobs()?
                        .get(&name)
                        .and_then(|job| job.process())
                        .is_some_and(|process| matches!(process.export, esdiag::job::model::ExportTarget::Stdout));
                    let outcome = esdiag::job::handle_job_run(&name).await?;
                    if stdout_owned {
                        Ok(CommandResult::stream())
                    } else {
                        Ok(CommandResult::outcome(job_outcome(name, outcome)))
                    }
                }
                JobCommands::Delete { name } => {
                    esdiag::job::handle_job_delete(&name)?;
                    Ok(CommandResult::outcome(CliOutcome::JobDeleted { name }))
                }
            },
        }
    } else {
        #[cfg(all(feature = "server", feature = "desktop"))]
        {
            // Set up communication channel to tell the server when to shut down
            let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

            // Tauri desktop wrapper logic
            tauri::Builder::default()
                .plugin(tauri_plugin_opener::init())
                .setup(|app| {
                    use tauri::Manager;

                    let handle = app.handle().clone();

                    tauri::async_runtime::spawn(async move {
                        let settings = esdiag::data::Settings::load().unwrap_or_default();

                        let exporter = if let Some(target) = &settings.active_target {
                            if let Ok(host) =
                                esdiag::data::KnownHost::get_known(target).ok_or_else(|| eyre::eyre!("Host not found"))
                            {
                                if let Ok(uri) = Uri::try_from(host) {
                                    Exporter::try_from(uri).unwrap_or_default()
                                } else {
                                    Exporter::default()
                                }
                            } else {
                                Exporter::default()
                            }
                        } else {
                            Exporter::default()
                        };

                        let kibana_url = settings.kibana_url.unwrap_or_else(|| {
                            let url = esdiag::env::get_string("ESDIAG_KIBANA_URL")
                                .unwrap_or_else(|_| "http://localhost:5601".to_string());
                            esdiag::env::append_kibana_space(&url)
                        });

                        let (mut server, bound_addr) =
                            match Server::start([127, 0, 0, 1], 0, exporter, kibana_url, RuntimeMode::User).await {
                                Ok(res) => res,
                                Err(e) => {
                                    tracing::error!("Failed to start embedded server: {}", e);
                                    return;
                                }
                            };

                        let url = format!("http://localhost:{}", bound_addr.port());

                        if let Ok(url) = tauri::Url::parse(&url) {
                            // Wait a tiny bit to ensure server is ready
                            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                            if let Some(window) = handle.get_webview_window("main") {
                                window.on_window_event(|event| {
                                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                                        // Custom close logic if needed
                                    }
                                });
                                let _ = window.navigate(url);
                                let _ = window.set_focus();
                            }
                        }

                        // Wait for Tauri exit signal
                        let _ = shutdown_rx.recv().await;
                        server.shutdown().await;
                    });

                    Ok(())
                })
                .on_window_event(move |_window, event| {
                    use tauri::WindowEvent;
                    if let WindowEvent::Destroyed = event {
                        // All windows closed, signal the server to shut down
                        let _ = shutdown_tx.try_send(());
                    }
                })
                .run(tauri::generate_context!())
                .expect("error while running tauri application");

            Ok(CommandResult::stream())
        }
        #[cfg(not(all(feature = "server", feature = "desktop")))]
        {
            use clap::CommandFactory;
            let mut cmd = Cli::command();
            cmd.print_help()?;
            Err(eyre!(
                "No command provided. If you want to use the Desktop UI, compile with the 'desktop' feature."
            ))
        }
    }
}

#[cfg(feature = "keystore")]
fn derive_collect_job(
    host: &str,
    output: &str,
    diagnostic_type: &str,
    upload_id: Option<&str>,
    identifiers: Identifiers,
) -> Result<esdiag::data::Job> {
    let builder = esdiag::data::Job::builder()
        .identifiers(identifiers)
        .collect_from(host)?
        .diagnostic_type(diagnostic_type);

    match upload_id {
        Some(upload_id) => builder.upload_to(upload_id),
        None => builder.collect_to(output),
    }
}

#[cfg(feature = "keystore")]
fn derive_process_job(input: &str, output: Option<&str>, identifiers: Identifiers) -> Result<esdiag::data::Job> {
    let output = output.ok_or_else(|| eyre!("Saved jobs require an explicit process output target"))?;
    let output_uri = Uri::try_from(output.to_string())?;
    ensure_uri_role(&output_uri, HostRole::Send, "save-job output")?;
    let output = esdiag::data::JobOutput::from_cli_target(output)?;
    esdiag::data::Job::builder()
        .identifiers(identifiers)
        .collect_from(input)?
        .process_to(output)
}

fn sources_application_key(application: Application) -> Result<&'static str> {
    esdiag::processor::diagnostic::data_source::source_application_key(application).map_err(|_| {
        eyre!(
            "--sources is only supported for Elasticsearch, Kibana, and Logstash inputs, got {}",
            application
        )
    })
}

async fn detect_sources_application_for_process(input_uri: &Uri, receiver: &Receiver) -> Result<Application> {
    match input_uri {
        Uri::KnownHost(host) | Uri::ElasticCloudAdmin(host) | Uri::ElasticGovCloudAdmin(host) => host
            .app()
            .ok_or_else(|| eyre!("--sources is only supported for application diagnostics")),
        _ => receiver
            .try_get_manifest_from_files()
            .await?
            .application()
            .ok_or_else(|| eyre!("--sources is only supported for application diagnostics")),
    }
}

#[cfg(all(feature = "server", unix))]
async fn wait_for_shutdown_signal() -> Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutting down server (Ctrl+C)...");
        }
        _ = async {
            let mut term_signal = signal(SignalKind::terminate())
                .map_err(|e| eyre!("Failed to install SIGTERM handler: {}", e))?;
            term_signal.recv().await;
            tracing::info!("Shutting down server (SIGTERM)...");
            Ok::<_, eyre::Report>(())
        } => {}
    }

    Ok(())
}

#[cfg(all(feature = "server", not(unix)))]
async fn wait_for_shutdown_signal() -> Result<()> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|e| eyre!("Failed to install Ctrl+C handler: {}", e))?;
    tracing::info!("Shutting down server (Ctrl+C)...");
    Ok(())
}

fn expected_secret_auth(
    username: Option<String>,
    password: Option<String>,
    apikey: Option<String>,
) -> Result<Option<SecretAuth>> {
    match (apikey, username, password) {
        (None, None, None) => Ok(None),
        (Some(apikey), None, None) => Ok(Some(SecretAuth::apikey(apikey))),
        (None, Some(username), Some(password)) => Ok(Some(SecretAuth::basic(username, password))),
        _ => Err(eyre!(
            "Invalid auth options: use either --apikey or --user with --password"
        )),
    }
}

fn build_host_cli_update(args: HostMutationArgs) -> KnownHostCliUpdate {
    args.into()
}

fn infer_product_from_url(url: &Url) -> Option<Application> {
    match url.port_or_known_default() {
        Some(9200) => Some(Application::Elasticsearch),
        Some(5601) => Some(Application::Kibana),
        Some(9600) => Some(Application::Logstash),
        _ => {
            let mut segments = url.path_segments()?.filter(|segment| !segment.is_empty());
            if matches!(segments.next(), Some("api"))
                && matches!(segments.next(), Some("v1"))
                && matches!(segments.next(), Some("deployments"))
            {
                let _deployment_id = segments.next()?;
                return Application::from_str(segments.next()?).ok();
            }
            None
        }
    }
}

fn build_host_from_definition(
    app: Option<Application>,
    target: &str,
    use_url_template: bool,
    update: &KnownHostCliUpdate,
    secret_auth: Option<SecretAuth>,
) -> Result<KnownHost> {
    let mut builder = if use_url_template {
        KnownHostBuilder::new_template(target.to_string())
    } else {
        let url = Url::parse(target).map_err(|err| eyre!("Invalid host target URL: {err}"))?;
        let inferred_app = infer_product_from_url(&url);
        let app = app
            .or(inferred_app)
            .ok_or_else(|| eyre!("Target '{target}' does not determine the app. Rerun with `--app <app>`."))?;
        KnownHostBuilder::new(url).application(app)
    }
    .accept_invalid_certs(update.accept_invalid_certs.unwrap_or(false))
    .legacy_credentials(update.apikey.clone(), update.username.clone(), update.password.clone())
    .secret(update.secret.clone());
    if let Some(app) = app {
        builder = builder.application(app);
    }
    if let Some(roles) = update.roles.clone() {
        builder = builder.roles(roles);
    }
    match secret_auth {
        Some(secret_auth) => builder.build_with_secret_auth(secret_auth),
        None => builder.build(),
    }
}

fn maybe_materialize_template_target(target: &str) -> Result<Option<KnownHost>> {
    KnownHost::resolve_template_reference(target)
}

async fn save_host(name: &str, host: KnownHost, action: &str, validate_connection: bool) -> Result<String> {
    if validate_connection {
        let uri = Uri::try_from(host.clone())?;
        let validation_summary = validate_host_connection(name, uri).await?;
        let hostfile = host.save(name)?;
        tracing::info!("Host {name} successfully saved to {hostfile}");
        return Ok(format!("{validation_summary}\nHost '{name}' {action} in {hostfile}"));
    }
    let hostfile = host.save(name)?;
    tracing::info!("Host {name} successfully saved to {hostfile}");
    Ok(format!("Host '{name}' {action} in {hostfile}"))
}

fn legacy_host_command_error(args: &[String]) -> eyre::Report {
    let attempted = if args.is_empty() {
        "esdiag host".to_string()
    } else {
        format!("esdiag host {}", args.join(" "))
    };
    eyre!(
        "Legacy positional host syntax is no longer supported. Use `esdiag host add <name> <target> [--app <app>]` to create a host, `esdiag host update <name>` to modify one, `esdiag host remove <name>` to delete one, `esdiag host list` to inspect saved hosts, or `esdiag host auth <target>` to test a saved host or resolved template reference. Received: `{attempted}`"
    )
}

fn cleanup_settings_after_host_delete(name: &str) -> Result<()> {
    let mut settings = Settings::load()?;
    if settings.active_target.as_deref() != Some(name) {
        return Ok(());
    }

    let hosts = KnownHost::parse_hosts_yml()?;
    settings.active_target = hosts.keys().next().cloned();
    settings.save()
}

fn delete_host_from_cli(name: &str) -> Result<String> {
    let path = KnownHost::remove_saved(name)?;
    if let Err(err) = cleanup_settings_after_host_delete(name) {
        eprintln!(
            "Warning: host '{}' was removed, but failed to update settings: {err}",
            name
        );
    }
    Ok(path)
}
fn resolve_secret_input(
    username: Option<String>,
    password: Option<String>,
    apikey: Option<String>,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    resolve_secret_input_with_prompt(username, password, apikey, prompt_missing_secret_value)
}

fn resolve_secret_input_with_prompt<F>(
    username: Option<String>,
    password: Option<String>,
    apikey: Option<String>,
    mut prompt_secret: F,
) -> Result<(Option<String>, Option<String>, Option<String>)>
where
    F: FnMut(&str) -> Result<String>,
{
    let requested_apikey_prompt = apikey.as_ref().is_some_and(|value| value.trim().is_empty());
    let requested_password_prompt = password.as_ref().is_some_and(|value| value.trim().is_empty());
    let username = normalize_optional_secret_arg(username);
    let mut password = normalize_optional_secret_arg(password);
    let mut apikey = normalize_optional_secret_arg(apikey);
    if requested_apikey_prompt {
        apikey = Some(prompt_secret("Enter secret API key: ")?);
    }
    match (&apikey, &username, &password) {
        (Some(_), None, None) => Ok((None, None, apikey)),
        (None, Some(_), Some(_)) => Ok((username, password, None)),
        (None, Some(_), None) => {
            if requested_password_prompt || password.is_none() {
                password = Some(prompt_secret("Enter secret password: ")?);
            }
            Ok((username, password, None))
        }
        (None, None, Some(_)) => Err(eyre!(
            "Invalid auth options: use either --apikey or --user with --password"
        )),
        (None, None, None) => Err(eyre!(
            "Invalid auth options: use either --apikey or --user with --password"
        )),
        (Some(_), _, _) => Ok((None, None, apikey)),
    }
}

fn normalize_optional_secret_arg(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn prompt_missing_secret_value(prompt: &str) -> Result<String> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(eyre!(
            "Required secret value was not provided and no interactive terminal is available."
        ));
    }
    let value = rpassword::prompt_password(prompt)?;
    if value.is_empty() {
        return Err(eyre!("Required secret value was not provided."));
    }
    Ok(value)
}

fn prompt_confirm(message: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Ok(false);
    }
    print!("{message}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let answer = line.trim().to_ascii_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}

fn prompt_confirm_default_yes(message: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Ok(false);
    }
    print!("{message}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let answer = line.trim().to_ascii_lowercase();
    Ok(!matches!(answer.as_str(), "n" | "no"))
}

fn prompt_new_keystore_password() -> Result<String> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(eyre!("A new keystore password requires an interactive terminal."));
    }
    let password = rpassword::prompt_password("Enter new keystore password: ")?;
    if password.is_empty() {
        return Err(eyre!("Keystore password cannot be empty."));
    }
    let confirm = rpassword::prompt_password("Confirm new keystore password: ")?;
    if password != confirm {
        return Err(eyre!("Keystore password confirmation did not match."));
    }
    Ok(password)
}

fn unlock_keystore(ttl: Duration) -> Result<std::path::PathBuf> {
    if keystore_exists()? {
        let keystore_password = get_password_for_secret_commands()?;
        validate_existing_keystore_password(&keystore_password)?;
        return write_unlock_lease(&keystore_password, ttl);
    }
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(eyre!(
            "No keystore exists and no interactive terminal is available to create one."
        ));
    }
    if !prompt_confirm("No keystore exists. Create one now? [y/N]: ")? {
        return Err(eyre!("Keystore unlock cancelled."));
    }
    let keystore_password = prompt_new_keystore_password()?;
    create_keystore(&keystore_password)?;
    write_unlock_lease(&keystore_password, ttl)
}

async fn run_init_wizard() -> Result<CommandResult> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(eyre!("`esdiag init` requires an interactive controlling terminal."));
    }

    let initial = inspect_onboarding()?;
    println!("ESDiag first-run initialization");
    let mut output_name_for_defaults = esdiag::data::ApplicationConfig::load()?.output.default;
    let mut output_url_for_defaults = output_name_for_defaults
        .as_ref()
        .and_then(KnownHost::get_known)
        .and_then(|host| host.concrete_url().map(Url::to_string));
    let mut most_recent_collect_host = None;
    if initial.is_complete() && !prompt_confirm("A complete configuration already exists. Replace values? [y/N]: ")? {
        return Ok(CommandResult::outcome(initialization_outcome()?));
    }

    let config = esdiag::data::ApplicationConfig::load()?;
    let replace_user = match config.user.as_deref() {
        Some(user) if !prompt_confirm_default_yes(&format!("Resume configuring user: {user} [Y/n]: "))? => {
            prompt_confirm("Replace existing configuration? [y/N]: ")?
        }
        Some(_) => false,
        None => true,
    };
    if replace_user {
        save_user(prompt_with_default(
            "Default diagnostic user",
            &default_diagnostic_user(),
        )?)?;
    }

    let config = esdiag::data::ApplicationConfig::load()?;
    let configure_cluster = if config.output.default.is_some() {
        prompt_confirm("Reconfigure the default diagnostics cluster? [y/N]: ")?
    } else {
        prompt_confirm_default_yes("Configure a diagnostics cluster now? [Y/n]: ")?
    };
    if configure_cluster {
        unlock_keystore(default_unlock_ttl())?;
        let keystore_password = get_password_for_secret_commands()?;
        let local_preset = detected_esdiag_local_preset();
        let output_name = prompt_with_default("Output host name", "localhost")?;
        let output_url = prompt_url_with_default(
            "Output Elasticsearch URL",
            local_preset
                .as_ref()
                .map_or("http://localhost:9200", |preset| preset.elasticsearch_url.as_str()),
        )?;
        let viewer_name = format!("{output_name}-kb");
        let viewer_url = prompt_url_with_default(
            "Output Kibana URL",
            local_preset
                .as_ref()
                .map_or("http://localhost:5601", |preset| preset.kibana_url.as_str()),
        )?;
        let secret_id = prompt_with_default("Shared output credential name", &output_name)?;
        let auth = prompt_api_key("Output", local_preset.as_ref())?;
        upsert_secret_auth(&secret_id, auth.clone(), &keystore_password)?;
        let output_candidate = KnownHostBuilder::new(output_url.clone())
            .application(Application::Elasticsearch)
            .roles(vec![HostRole::Send])
            .viewer(Some(viewer_name.clone()))
            .secret(Some(secret_id.clone()))
            .build_with_secret_auth(auth.clone())?;
        let viewer_candidate = KnownHostBuilder::new(viewer_url.clone())
            .application(Application::Kibana)
            .roles(vec![HostRole::View])
            .secret(Some(secret_id.clone()))
            .build_with_secret_auth(auth.clone())?;
        let output_client = Client::try_from(Uri::try_from(output_candidate)?)?;
        let output_valid = match output_client.test_connection().await {
            Ok(_) => true,
            Err(err) => {
                tracing::warn!("Output Elasticsearch validation failed: {err}");
                false
            }
        };
        let viewer_client = Client::try_from(Uri::try_from(viewer_candidate)?)?;
        let viewer_valid = match viewer_client.test_connection().await {
            Ok(_) => true,
            Err(err) => {
                tracing::warn!("Output Kibana validation failed: {err}");
                false
            }
        };
        output_name_for_defaults = Some(output_name.clone());
        output_url_for_defaults = Some(output_url.to_string());
        save_output_deployment(
            OutputDeploymentInput {
                output_name,
                output_url,
                viewer_name,
                viewer_url,
                secret_id,
                auth,
            },
            &keystore_password,
        )?;

        let mut output_config = esdiag::data::ApplicationConfig::load()?;
        output_config.output.authenticated_on = (output_valid && viewer_valid).then(|| chrono::Utc::now().to_rfc3339());
        #[cfg(feature = "setup")]
        if output_valid && viewer_valid && prompt_confirm("Install or update ESDiag output assets now? [y/N]: ")? {
            setup::assets(&output_client).await?;
            setup::ensure_agent_builder_license(&output_client).await?;
            setup::assets(&viewer_client).await?;
            output_config.output.assets_version = Some(env!("CARGO_PKG_VERSION").to_string());
        } else if !output_valid || !viewer_valid {
            output_config.output.assets_version = None;
            tracing::warn!("Skipping output asset setup until both endpoints validate successfully");
        }
        #[cfg(not(feature = "setup"))]
        if !output_valid || !viewer_valid {
            tracing::warn!("Output asset setup is unavailable and endpoint validation did not complete successfully");
        }
        output_config.save()?;
    }

    if !initial.collect_host_configured || prompt_confirm("Add or replace a collect host? [y/N]: ")? {
        loop {
            let collect_name_default = output_name_for_defaults.clone().unwrap_or_else(|| "source".to_string());
            let name = prompt_with_default("Collect host name", &collect_name_default)?;
            let url = prompt_url_with_default(
                "Collect Elasticsearch URL",
                output_url_for_defaults.as_deref().unwrap_or("http://localhost:9200"),
            )?;
            let reuse_output_secret = output_name_for_defaults.is_some()
                && prompt_confirm_default_yes("Reuse the output credential for this host? [Y/n]: ")?;
            let secret_id = if reuse_output_secret {
                esdiag::data::ApplicationConfig::load()?
                    .output
                    .default
                    .and_then(|output| KnownHost::get_known(&output))
                    .and_then(|host| host.secret)
            } else {
                Some(prompt_with_default("Collect-host credential name", &name)?)
            };
            let auth = if reuse_output_secret {
                None
            } else {
                Some(prompt_api_key("Collect-host", detected_esdiag_local_preset().as_ref())?)
            };
            let keystore_password = if auth.is_some() {
                unlock_keystore(default_unlock_ttl())?;
                Some(get_password_for_secret_commands()?)
            } else {
                None
            };
            save_collect_host(
                CollectHostInput {
                    name: name.clone(),
                    app: Application::Elasticsearch,
                    url,
                    secret_id,
                    auth,
                },
                keystore_password.as_deref(),
            )?;
            most_recent_collect_host = Some(name);
            if !prompt_confirm("Add another collect host? [y/N]: ")? {
                break;
            }
        }
    }

    let config = esdiag::data::ApplicationConfig::load()?;
    if config.job.default.is_none() || prompt_confirm("Replace the default saved job? [y/N]: ")? {
        let collect_host_default = most_recent_collect_host
            .or_else(first_saved_collect_host)
            .ok_or_else(|| eyre!("Add a collect host before creating the default job"))?;
        let collect_host = prompt_with_default("Collect host for the default job", &collect_host_default)?;
        let collect_job_name = format!("{collect_host}-collect");
        let collect_job = esdiag::data::Job::builder()
            .collect_from(collect_host.clone())?
            .collect_to(format!("diagnostics/{collect_host}"))?;
        save_default_job(collect_job_name, collect_job)?;
        if let Some(output) = esdiag::data::ApplicationConfig::load()?.output.default {
            let process_job_name = format!("{collect_host}-process-{output}");
            save_default_processing_job(process_job_name, collect_host)?;
        }
    }

    let readiness = inspect_onboarding()?;
    if !readiness.is_complete() {
        return Err(eyre!("Initialization did not produce a complete reusable workflow."));
    }
    Ok(CommandResult::outcome(initialization_outcome()?))
}

fn initialization_outcome() -> Result<CliOutcome> {
    let config = esdiag::data::ApplicationConfig::load()?;
    Ok(CliOutcome::InitializationCompleted {
        user: config
            .user
            .ok_or_else(|| eyre!("Initialization completed without a configured user"))?,
        output: config.output.default,
        job: config
            .job
            .default
            .ok_or_else(|| eyre!("Initialization completed without a default job"))?,
    })
}

fn prompt_required(message: &str) -> Result<String> {
    print!("{message}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let value = line.trim().to_string();
    if value.is_empty() {
        return Err(eyre!("A value is required."));
    }
    Ok(value)
}

fn prompt_with_default(message: &str, default: &str) -> Result<String> {
    print!("{message} [{default}]: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let value = line.trim();
    Ok(if value.is_empty() {
        default.to_string()
    } else {
        value.to_string()
    })
}

fn prompt_url_with_default(message: &str, default: &str) -> Result<Url> {
    Url::parse(&prompt_with_default(message, default)?).map_err(Into::into)
}

fn first_saved_collect_host() -> Option<String> {
    KnownHost::parse_hosts_yml()
        .ok()?
        .into_iter()
        .find(|(_, host)| host.has_role(HostRole::Collect))
        .map(|(name, _)| name)
}

#[derive(Clone)]
struct EsdiagLocalPreset {
    elasticsearch_url: String,
    kibana_url: String,
    apikey: Option<String>,
}

fn detected_esdiag_local_preset() -> Option<EsdiagLocalPreset> {
    let state_dir = std::env::var_os("ESDIAG_LOCAL_DIR")
        .map(PathBuf::from)
        .or_else(default_esdiag_local_state_dir)?;
    let contents = std::fs::read_to_string(state_dir.join(".env")).ok()?;
    let values = contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            (!line.is_empty() && !line.starts_with('#'))
                .then(|| line.split_once('='))
                .flatten()
        })
        .collect::<std::collections::HashMap<_, _>>();
    let elasticsearch_port = values.get("ESDIAG_ELASTICSEARCH_PORT").copied().unwrap_or("9200");
    let kibana_port = values.get("ESDIAG_KIBANA_PORT").copied().unwrap_or("5601");
    Some(EsdiagLocalPreset {
        elasticsearch_url: format!("http://localhost:{elasticsearch_port}"),
        kibana_url: format!("http://localhost:{kibana_port}"),
        apikey: values
            .get("ESDIAG_OUTPUT_APIKEY")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty() && *value != "pending")
            .map(str::to_string),
    })
}

fn default_esdiag_local_state_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let home = std::env::var_os("USERPROFILE")?;
    #[cfg(not(target_os = "windows"))]
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".esdiag/local"))
}

fn prompt_api_key(label: &str, local_preset: Option<&EsdiagLocalPreset>) -> Result<SecretAuth> {
    let default = if local_preset.and_then(|preset| preset.apikey.as_ref()).is_some() {
        "3"
    } else {
        "4"
    };
    loop {
        println!("{label} API key source:");
        println!("  1. Read from a file");
        println!("  2. Read from an environment variable");
        println!("  3. Read from detected esdiag-local configuration");
        println!("  4. Paste securely");
        let source = prompt_with_default("Selection", default)?;
        let apikey = match source.as_str() {
            "1" | "file" => prompt_required("API key file path: ").and_then(|path| read_api_key_file(&path)),
            "2" | "environment" | "env" => {
                let name = prompt_with_default("API key environment variable", "ESDIAG_OUTPUT_APIKEY")?;
                std::env::var(&name).map_err(|_| eyre!("{name} is not set"))
            }
            "3" | "esdiag-local" | "local" => local_preset
                .and_then(|preset| preset.apikey.clone())
                .ok_or_else(|| eyre!("No usable ESDiag local API key was detected")),
            "4" | "paste" => prompt_missing_secret_value(&format!("Enter {label} API key: ")),
            _ => Err(eyre!("Choose API key source 1, 2, 3, or 4")),
        };
        let apikey = match apikey {
            Ok(apikey) => apikey.trim().to_string(),
            Err(err) => {
                print_api_key_source_warning(&format!("Unable to read API key source: {err}. Please try again."));
                continue;
            }
        };
        if apikey.is_empty() || apikey == "pending" || apikey.contains('\n') || apikey.contains('\r') {
            print_api_key_source_warning(
                "The selected API key source did not provide one usable API key. Please try again.",
            );
            continue;
        }
        return Ok(SecretAuth::apikey(apikey));
    }
}

fn print_api_key_source_warning(message: &str) {
    if std::io::stderr().is_terminal() {
        eprintln!("\x1b[33mWarning:\x1b[0m {message}");
    } else {
        eprintln!("Warning: {message}");
    }
}

fn read_api_key_file(path: &str) -> Result<String> {
    let contents = std::fs::read_to_string(path)?;
    let mut lines = contents.lines().map(str::trim).filter(|line| !line.is_empty());
    let apikey = lines.next().ok_or_else(|| eyre!("API key file is empty"))?.to_string();
    if lines.next().is_some() {
        return Err(eyre!("API key file must contain exactly one non-empty line"));
    }
    Ok(apikey)
}

fn default_diagnostic_user() -> String {
    detect_os_email()
        .or_else(|| std::env::var("EMAIL").ok().filter(|email| email.contains('@')))
        .or_else(|| std::env::var("USER").ok())
        .or_else(|| std::env::var("USERNAME").ok())
        .unwrap_or_else(|| "user".to_string())
}

#[cfg(target_os = "macos")]
fn detect_os_email() -> Option<String> {
    let username = std::env::var("USER").ok()?;
    let output = Command::new("dscl")
        .args([".", "-read", &format!("/Users/{username}"), "EMailAddress"])
        .output()
        .ok()?;
    String::from_utf8(output.stdout)
        .ok()?
        .split_whitespace()
        .find(|value| value.contains('@'))
        .map(str::to_string)
}

#[cfg(target_os = "windows")]
fn detect_os_email() -> Option<String> {
    let output = Command::new("whoami").arg("/upn").output().ok()?;
    String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .map(str::trim)
        .find(|value| value.contains('@'))
        .map(str::to_string)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn detect_os_email() -> Option<String> {
    None
}

fn format_epoch(epoch_seconds: i64) -> String {
    chrono::DateTime::from_timestamp(epoch_seconds, 0)
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(|| epoch_seconds.to_string())
}

fn format_remaining_duration(expires_at_epoch: i64) -> String {
    format_remaining_duration_from(chrono::Utc::now().timestamp(), expires_at_epoch)
}

fn format_remaining_duration_from(now_epoch: i64, expires_at_epoch: i64) -> String {
    let remaining = expires_at_epoch.saturating_sub(now_epoch);
    let duration = Duration::from_secs(remaining.max(0) as u64);
    let days = duration.as_secs() / 86_400;
    let hours = (duration.as_secs() % 86_400) / 3_600;
    let minutes = (duration.as_secs() % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h remaining")
    } else if hours > 0 {
        format!("{hours}h {minutes}m remaining")
    } else {
        format!("{minutes}m remaining")
    }
}

#[cfg(test)]
fn format_keystore_lock_status(status: &esdiag::data::UnlockStatus) -> String {
    format_keystore_lock_status_at(chrono::Utc::now().timestamp(), status)
}

#[cfg(test)]
fn format_keystore_lock_status_at(now_epoch: i64, status: &esdiag::data::UnlockStatus) -> String {
    if status.unlock_active {
        if let Some(expires_at_epoch) = status.expires_at_epoch {
            return format!(
                "Keystore: unlocked until {} ({})",
                format_epoch(expires_at_epoch),
                format_remaining_duration_from(now_epoch, expires_at_epoch)
            );
        }
        return "Keystore: unlocked".to_string();
    }

    "Keystore: locked".to_string()
}

#[cfg(test)]
fn colorize_keystore_lock_status(status: &str, colorize: bool) -> String {
    if !colorize {
        return status.to_string();
    }

    if status.contains("Keystore: unlocked") {
        return status.replacen("unlocked", "\x1b[32munlocked\x1b[0m", 1);
    }
    if status.contains("Keystore: locked") {
        return status.replacen("locked", "\x1b[31mlocked\x1b[0m", 1);
    }
    status.to_string()
}

fn ensure_host_role(host: &KnownHost, role: HostRole, context: &str) -> Result<()> {
    if host.has_role(role.clone()) {
        Ok(())
    } else {
        Err(eyre!(
            "Host role validation failed for {context}: required role '{}' not present",
            role
        ))
    }
}

fn ensure_uri_role(uri: &Uri, role: HostRole, context: &str) -> Result<()> {
    match uri {
        Uri::KnownHost(host) | Uri::ElasticCloudAdmin(host) | Uri::ElasticGovCloudAdmin(host) => {
            ensure_host_role(host, role, context)
        }
        _ => Ok(()),
    }
}

fn resolve_host_secret_auth(secret_id: Option<&str>) -> Result<Option<SecretAuth>> {
    let Some(secret_id) = secret_id else {
        return Ok(None);
    };

    let keystore_password = get_password_for_secret_commands()?;
    let secret_auth = resolve_secret_auth(secret_id, &keystore_password)?
        .ok_or_else(|| eyre!("Secret '{secret_id}' was not found in keystore"))?;
    Ok(Some(secret_auth))
}

fn resolve_same_name_host_secret_auth(host_name: &str) -> Result<Option<SecretAuth>> {
    let Ok(keystore_password) = get_password_for_secret_commands() else {
        return Ok(None);
    };
    resolve_secret_auth(host_name, &keystore_password)
}

fn host_connection_uses_receiver(uri: &Uri) -> bool {
    matches!(uri, Uri::ElasticCloudAdmin(_) | Uri::ElasticGovCloudAdmin(_))
}

async fn validate_host_connection(name: &str, uri: Uri) -> Result<String> {
    if host_connection_uses_receiver(&uri) {
        let receiver = Receiver::try_from(uri)?;
        if receiver.is_connected().await {
            let summary = format!("Host {name}: connected to Elastic Cloud Admin proxy");
            tracing::info!("{summary}");
            return Ok(summary);
        }

        tracing::error!("Host connection: FAILED ❌ Elastic Cloud Admin proxy connection failed");
        tracing::warn!("Check your URL, certificates, and secret credentials!");
        return Err(eyre!("Host connection failed"));
    }

    match Client::try_from(uri)?.test_connection().await {
        Ok(message) => {
            let summary = format!("Host {name}: {message}");
            tracing::info!("{summary}");
            Ok(summary)
        }
        Err(message) => {
            tracing::error!("Host connection: FAILED ❌ {}", &message);
            tracing::warn!("Check your URL and certificates!");
            Err(eyre!("Host connection failed"))
        }
    }
}

fn should_error_for_missing_subcommand(arg_count: usize, has_no_command: bool) -> bool {
    arg_count > 1 && has_no_command
}

fn clear_last_run_files() -> Result<()> {
    let home_dir = match std::env::consts::OS {
        "windows" => std::env::var("USERPROFILE")?,
        "linux" | "macos" => std::env::var("HOME")?,
        os => return Err(eyre!("Unknown home directory for operating system: {os} ")),
    };
    tracing::debug!("Home directory is: {home_dir}");
    let last_run = std::path::PathBuf::from(home_dir).join(".esdiag/last_run");
    if !last_run.exists() {
        std::fs::create_dir_all(&last_run)?;
    }
    let files = vec![
        "bulk_errors.ndjson",
        "diagnostic.json",
        "report.json",
        "responses.ndjson",
    ];
    for file in files {
        let file = last_run.join(file);
        tracing::debug!("Removing {}", &file.display());
        // Ignore "file not found" errors on delete
        let _ = std::fs::remove_file(file);
    }
    Ok(())
}

#[cfg(feature = "server")]
fn resolve_serve_runtime_mode(mode: Option<RuntimeMode>) -> Result<RuntimeMode> {
    if let Some(mode) = mode {
        return Ok(mode);
    }
    match std::env::var("ESDIAG_MODE") {
        Ok(value) => RuntimeMode::from_env(&value),
        Err(std::env::VarError::NotPresent) => Ok(RuntimeMode::User),
        Err(err) => Err(eyre!("Failed to read ESDIAG_MODE: {err}")),
    }
}

#[cfg(feature = "server")]
fn resolve_serve_exporter(output: Option<String>) -> Result<Exporter> {
    Exporter::try_from(Uri::try_from(output)?)
}

#[cfg(test)]
mod tests {
    use super::{
        Cli, Commands, HostCommands, KeystoreCommands, classify_failure, colorize_keystore_lock_status,
        command_owns_stdout, format_keystore_lock_status, format_keystore_lock_status_at,
        format_remaining_duration_from, host_connection_uses_receiver, is_agent_mode, resolve_host_secret_auth,
        resolve_secret_input_with_prompt, resolve_tracing_filter, should_error_for_missing_subcommand,
        structured_failure,
    };
    #[cfg(feature = "keystore")]
    use super::{derive_collect_job, derive_process_job};
    #[cfg(feature = "server")]
    use super::{resolve_serve_exporter, resolve_serve_runtime_mode};
    use clap::Parser;
    use esdiag::data::{Application, HostRole, KnownHost, SecretAuth, UnlockStatus, Uri, upsert_secret_auth};
    #[cfg(feature = "server")]
    use esdiag::server::RuntimeMode;
    use esdiag::{
        cli_output::{CliFailureCategory, OutputFormat},
        receiver::{
            ElasticCloudAdminRequestError, ElasticsearchRequestError, KibanaRequestError, LogstashRequestError,
        },
    };
    #[cfg(feature = "keystore")]
    use esdiag::{
        job::model::{ExportTarget, Input, Job, Process},
        processor::Identifiers,
    };
    use std::sync::Mutex;
    use tempfile::TempDir;
    use url::Url;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn setup_env() -> TempDir {
        let tmp = TempDir::new().expect("temp dir");
        let hosts = tmp.path().join("hosts.yml");
        let keystore = tmp.path().join("secrets.yml");
        unsafe {
            std::env::set_var("ESDIAG_HOSTS", &hosts);
            std::env::set_var("ESDIAG_KEYSTORE", &keystore);
            std::env::set_var("ESDIAG_KEYSTORE_PASSWORD", "pw");
        }
        tmp
    }

    #[test]
    fn no_args_and_no_command_allows_desktop_path() {
        assert!(!should_error_for_missing_subcommand(1, true));
    }

    #[test]
    fn args_without_subcommand_errors() {
        assert!(should_error_for_missing_subcommand(2, true));
        assert!(should_error_for_missing_subcommand(3, true));
    }

    #[test]
    fn args_with_subcommand_does_not_error() {
        assert!(!should_error_for_missing_subcommand(2, false));
    }

    #[test]
    fn agent_flag_parses_long_and_short_forms() {
        let cli = Cli::parse_from(["esdiag", "--agent", "keystore", "status"]);
        assert!(cli.agent, "long --agent should enable agent mode");

        let cli = Cli::parse_from(["esdiag", "-a", "keystore", "status"]);
        assert!(cli.agent, "short -a should enable agent mode");
    }

    #[test]
    fn output_format_defaults_to_yaml_and_accepts_json() {
        let yaml = Cli::parse_from(["esdiag", "keystore", "status"]);
        assert_eq!(yaml.format, OutputFormat::Yaml);

        let json = Cli::parse_from(["esdiag", "--format", "json", "keystore", "status"]);
        assert_eq!(json.format, OutputFormat::Json);
    }

    #[test]
    fn unauthorized_elasticsearch_responses_are_authentication_failures() {
        let error = eyre::Report::new(ElasticsearchRequestError::new(
            elasticsearch::http::StatusCode::UNAUTHORIZED,
            "missing authentication credentials".to_string(),
            5,
            34,
        ));

        assert_eq!(classify_failure(&error), CliFailureCategory::AuthenticationFailed);
    }

    #[test]
    fn http_failures_include_status_type_and_reason() {
        let error = eyre::Report::new(ElasticsearchRequestError::new(
            elasticsearch::http::StatusCode::UNAUTHORIZED,
            r#"{"error":{"type":"security_exception","reason":"missing authentication credentials for REST request [api_key=secret]"},"status":401}"#
                .to_string(),
            5,
            133,
        ));
        let failure = structured_failure(&error);
        let value = serde_json::to_value(failure).expect("serialize failure");

        assert_eq!(value["category"], CliFailureCategory::AuthenticationFailed.as_str());
        assert_eq!(value["status"], 401);
        assert_eq!(value["type"], "security_exception");
        assert_eq!(
            value["reason"],
            "missing authentication credentials for REST request [api_key=secret]"
        );
        assert_eq!(
            value["message"],
            "The server rejected the request because authentication credentials are required."
        );
    }

    #[test]
    fn non_elasticsearch_http_failures_use_the_shared_response_projection() {
        let body = r#"{"error":{"type":"upstream_failure","reason":"remote dependency was unavailable"}}"#.to_string();
        let failures = [
            (
                eyre::Report::new(KibanaRequestError::new(
                    reqwest::StatusCode::TOO_MANY_REQUESTS,
                    body.clone(),
                    5,
                    80,
                )),
                429,
                CliFailureCategory::InvalidInput,
            ),
            (
                eyre::Report::new(LogstashRequestError::new(
                    reqwest::StatusCode::NOT_FOUND,
                    body.clone(),
                    5,
                    80,
                )),
                404,
                CliFailureCategory::NotFound,
            ),
            (
                eyre::Report::new(ElasticCloudAdminRequestError::new(
                    reqwest::StatusCode::BAD_GATEWAY,
                    body,
                    5,
                    80,
                )),
                502,
                CliFailureCategory::Internal,
            ),
        ];

        for (error, status, category) in failures {
            let value = serde_json::to_value(structured_failure(&error)).expect("serialize failure");
            assert_eq!(value["category"], category.as_str());
            assert_eq!(value["status"], status);
            assert_eq!(value["type"], "upstream_failure");
            assert_eq!(value["reason"], "remote dependency was unavailable");
        }
    }

    #[test]
    fn direct_process_stdout_is_not_replaced_with_a_terminal_outcome() {
        let cli = Cli::parse_from(["esdiag", "process", "input.zip", "-"]);
        assert!(command_owns_stdout(&cli));
    }

    #[cfg(feature = "keystore")]
    #[test]
    fn saved_job_stdout_is_not_replaced_with_a_terminal_outcome() {
        let _env_guard = env_lock().lock().expect("lock environment");
        let _tmp = setup_env();
        let job = Job::try_new(
            Identifiers::default(),
            Input::Collect {
                host: "prod".to_string(),
                diagnostic_type: "standard".to_string(),
                include: None,
                exclude: None,
            },
            None,
            Some(Process {
                selection: None,
                export: ExportTarget::Stdout,
            }),
            None,
        )
        .expect("valid streaming job");
        esdiag::job::save_job("stdout-job", job).expect("save job");

        let cli = Cli::parse_from(["esdiag", "job", "run", "stdout-job"]);
        assert!(command_owns_stdout(&cli));
    }

    #[test]
    fn host_add_parses_comma_delimited_role_values() {
        let cli = Cli::parse_from([
            "esdiag",
            "host",
            "add",
            "prod-es",
            "http://localhost:9200",
            "--app",
            "elasticsearch",
            "--roles",
            "collect,send",
        ]);
        match cli.command.expect("command") {
            Commands::Host {
                command: HostCommands::Add { args, .. },
            } => {
                assert_eq!(args.roles, Some(vec![HostRole::Collect, HostRole::Send]));
            }
            _ => panic!("expected host command"),
        }
    }

    #[test]
    fn parses_init_command() {
        let cli = Cli::try_parse_from(["esdiag", "init"]).expect("parse init");

        assert!(matches!(cli.command, Some(Commands::Init)));
    }

    #[test]
    fn host_update_accept_invalid_certs_cli_parses_explicit_bool_values() {
        let cli_true = Cli::parse_from(["esdiag", "host", "update", "prod-es", "--accept-invalid-certs", "true"]);
        match cli_true.command.expect("command") {
            Commands::Host {
                command: HostCommands::Update { args, .. },
            } => {
                assert_eq!(args.accept_invalid_certs, Some(true));
            }
            _ => panic!("expected host command"),
        }

        let cli_false = Cli::parse_from(["esdiag", "host", "update", "prod-es", "--accept-invalid-certs", "false"]);
        match cli_false.command.expect("command") {
            Commands::Host {
                command: HostCommands::Update { args, .. },
            } => {
                assert_eq!(args.accept_invalid_certs, Some(false));
            }
            _ => panic!("expected host command"),
        }
    }

    #[test]
    fn host_legacy_positional_syntax_is_captured_as_legacy_subcommand() {
        let cli = Cli::parse_from(["esdiag", "host", "prod-es", "--secret", "rotated"]);
        match cli.command.expect("command") {
            Commands::Host {
                command: HostCommands::Legacy(args),
            } => {
                assert_eq!(
                    args,
                    vec!["prod-es".to_string(), "--secret".to_string(), "rotated".to_string()]
                );
            }
            _ => panic!("expected legacy host command"),
        }
    }

    #[test]
    fn keystore_add_allows_missing_apikey_value_for_prompting() {
        let cli = Cli::parse_from(["esdiag", "keystore", "add", "prod-es", "--apikey"]);
        match cli.command.expect("command") {
            Commands::Keystore {
                command: KeystoreCommands::Add { apikey, .. },
            } => {
                assert_eq!(apikey, Some(String::new()));
            }
            _ => panic!("expected keystore add command"),
        }
    }

    #[test]
    fn keystore_update_allows_missing_password_value_for_prompting() {
        let cli = Cli::parse_from([
            "esdiag",
            "keystore",
            "update",
            "prod-es",
            "--user",
            "elastic",
            "--password",
        ]);
        match cli.command.expect("command") {
            Commands::Keystore {
                command: KeystoreCommands::Update { password, .. },
            } => {
                assert_eq!(password, Some(String::new()));
            }
            _ => panic!("expected keystore update command"),
        }
    }

    #[test]
    fn resolve_secret_input_uses_prompted_value_for_missing_apikey() {
        let mut prompts = Vec::new();
        let resolved = resolve_secret_input_with_prompt(None, None, Some(String::new()), |prompt| {
            prompts.push(prompt.to_string());
            Ok("prompted-api-key".to_string())
        })
        .expect("resolve secret input");

        assert_eq!(prompts, vec!["Enter secret API key: ".to_string()]);
        assert_eq!(resolved, (None, None, Some("prompted-api-key".to_string())));
    }

    #[test]
    fn remaining_duration_formats_hours_and_minutes() {
        assert_eq!(
            format_remaining_duration_from(1_700_000_000, 1_700_003_660),
            "1h 1m remaining"
        );
    }

    #[test]
    fn keystore_status_reports_locked_state() {
        let status = UnlockStatus {
            keystore_exists: true,
            unlock_active: false,
            expires_at_epoch: None,
            unlock_path: std::path::PathBuf::from("/tmp/keystore.unlock"),
        };

        assert_eq!(format_keystore_lock_status(&status), "Keystore: locked");
    }

    #[test]
    fn keystore_status_reports_unlock_expiry() {
        let status = UnlockStatus {
            keystore_exists: true,
            unlock_active: true,
            expires_at_epoch: Some(1_700_003_660),
            unlock_path: std::path::PathBuf::from("/tmp/keystore.unlock"),
        };

        assert_eq!(
            format_keystore_lock_status_at(1_700_000_000, &status),
            "Keystore: unlocked until 2023-11-14T23:14:20+00:00 (1h 1m remaining)"
        );
    }

    #[test]
    fn keystore_status_colorizes_locked_state_when_enabled() {
        assert_eq!(
            colorize_keystore_lock_status("Keystore: locked", true),
            "Keystore: \u{1b}[31mlocked\u{1b}[0m"
        );
    }

    #[test]
    fn keystore_status_colorizes_unlocked_state_when_enabled() {
        assert_eq!(
            colorize_keystore_lock_status("Keystore: unlocked until later", true),
            "Keystore: \u{1b}[32munlocked\u{1b}[0m until later"
        );
    }

    #[test]
    fn keystore_status_leaves_plain_text_when_color_disabled() {
        assert_eq!(
            colorize_keystore_lock_status("Keystore: locked", false),
            "Keystore: locked"
        );
    }

    #[test]
    fn collect_command_parses_upload_id() {
        let cli = Cli::parse_from(["esdiag", "collect", "prod-es", "diag-dir", "--upload", "abc123"]);
        match cli.command.expect("command") {
            Commands::Collect {
                host,
                output,
                upload_id,
                user,
                ..
            } => {
                assert_eq!(host, "prod-es");
                assert_eq!(output, "diag-dir");
                assert_eq!(upload_id, Some("abc123".to_string()));
                assert_eq!(user, None);
            }
            _ => panic!("expected collect command"),
        }
    }

    #[test]
    fn collect_command_keeps_user_short_option() {
        let cli = Cli::parse_from([
            "esdiag", "collect", "prod-es", "diag-dir", "-u", "elastic", "--upload", "abc123",
        ]);
        match cli.command.expect("command") {
            Commands::Collect { upload_id, user, .. } => {
                assert_eq!(user, Some("elastic".to_string()));
                assert_eq!(upload_id, Some("abc123".to_string()));
            }
            _ => panic!("expected collect command"),
        }
    }

    #[cfg(feature = "keystore")]
    #[test]
    fn collect_command_parses_save_job_flag() {
        let cli = Cli::parse_from(["esdiag", "collect", "prod-es", "diag-dir", "--save-job", "nightly-prod"]);
        match cli.command.expect("command") {
            Commands::Collect { save_job, .. } => {
                assert_eq!(save_job.as_deref(), Some("nightly-prod"));
            }
            _ => panic!("expected collect command"),
        }
    }

    #[cfg(feature = "keystore")]
    #[test]
    fn process_command_parses_save_job_flag() {
        let cli = Cli::parse_from([
            "esdiag",
            "process",
            "prod-es",
            "monitoring-es",
            "--save-job",
            "process-prod",
        ]);
        match cli.command.expect("command") {
            Commands::Process { save_job, .. } => {
                assert_eq!(save_job.as_deref(), Some("process-prod"));
            }
            _ => panic!("expected process command"),
        }
    }

    #[cfg(feature = "keystore")]
    #[test]
    fn derive_collect_job_requires_known_host_input() {
        let _guard = env_lock().lock().expect("env lock");
        let _tmp = setup_env();
        let err = match derive_collect_job(
            "https://example.com",
            "diag-dir",
            "standard",
            None,
            Identifiers::default(),
        ) {
            Ok(_) => panic!("non-host input should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("Jobs require a saved known host name as input")
        );
    }

    #[cfg(feature = "keystore")]
    #[test]
    fn derive_collect_job_uses_output_dir_without_save_dir() {
        let _guard = env_lock().lock().expect("env lock");
        let _tmp = setup_env();
        let host = KnownHost::new_no_auth(
            Application::Elasticsearch,
            Url::parse("http://localhost:9200").expect("valid url"),
            vec![HostRole::Collect],
            None,
            false,
        );
        host.save("prod-es").expect("save known host");

        let job = derive_collect_job("prod-es", "/tmp/esdiag-output", "support", None, Identifiers::default())
            .expect("derive collect job");

        assert!(job.process().is_none());
        assert!(job.send().is_none());
        assert_eq!(
            job.save().and_then(|save| save.dir.as_ref()),
            Some(&std::path::PathBuf::from("/tmp/esdiag-output"))
        );
    }

    #[cfg(feature = "keystore")]
    #[test]
    fn derive_process_job_requires_explicit_output() {
        let _guard = env_lock().lock().expect("env lock");
        let _tmp = setup_env();
        let host = KnownHost::new_no_auth(
            Application::Elasticsearch,
            Url::parse("http://localhost:9200").expect("valid url"),
            vec![HostRole::Collect],
            None,
            false,
        );
        host.save("prod-es").expect("save known host");
        let err = match derive_process_job("prod-es", None, Identifiers::default()) {
            Ok(_) => panic!("missing process output should be rejected"),
            Err(err) => err,
        };
        assert_eq!(err.to_string(), "Saved jobs require an explicit process output target");
    }

    #[test]
    fn upload_command_parses_file_and_upload_id() {
        let cli = Cli::parse_from(["esdiag", "upload", "diag.zip", "abc123"]);
        match cli.command.expect("command") {
            Commands::Upload {
                file_name,
                upload_id,
                api_url,
            } => {
                assert_eq!(file_name, "diag.zip");
                assert_eq!(upload_id, "abc123");
                assert_eq!(api_url, esdiag::uploader::DEFAULT_UPLOAD_API_URL);
            }
            _ => panic!("expected upload command"),
        }
    }

    #[test]
    fn agent_mode_auto_enables_from_claudecode_env() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("CLAUDECODE", "1");
        }

        let cli = Cli::parse_from(["esdiag", "keystore", "status"]);
        assert!(is_agent_mode(&cli));

        unsafe {
            std::env::remove_var("CLAUDECODE");
        }
    }

    #[test]
    fn debug_overrides_agent_warn_filter() {
        let cli = Cli {
            debug: true,
            agent: true,
            format: OutputFormat::Yaml,
            command: None,
        };

        assert_eq!(resolve_tracing_filter(&cli).to_string(), "debug");
    }

    #[test]
    fn agent_mode_uses_warn_filter_without_debug() {
        let cli = Cli {
            debug: false,
            agent: true,
            format: OutputFormat::Yaml,
            command: None,
        };

        assert_eq!(resolve_tracing_filter(&cli).to_string(), "warn");
    }

    #[test]
    fn elastic_cloud_admin_hosts_validate_via_receiver() {
        let uri = Uri::ElasticCloudAdmin(KnownHost::new_legacy_apikey(
            Application::Elasticsearch,
            Url::parse("https://admin.found.no/api/v1/deployments/test/elasticsearch/main-elasticsearch/proxy/")
                .expect("valid url"),
            vec![HostRole::Collect],
            None,
            false,
            Some("ada-admin".to_string()),
            None,
        ));

        assert!(host_connection_uses_receiver(&uri));
    }

    #[test]
    fn standard_known_hosts_validate_via_client() {
        let uri = Uri::KnownHost(KnownHost::new_no_auth(
            Application::Elasticsearch,
            Url::parse("http://localhost:9200").expect("valid url"),
            vec![HostRole::Collect],
            None,
            false,
        ));

        assert!(!host_connection_uses_receiver(&uri));
    }

    #[test]
    fn host_secret_auth_resolution_detects_apikey() {
        let _guard = env_lock().lock().expect("env lock");
        let _tmp = setup_env();
        upsert_secret_auth("api-secret", SecretAuth::apikey("secret-key"), "pw").expect("save api secret");

        let resolved = resolve_host_secret_auth(Some("api-secret")).expect("resolve auth");
        assert!(matches!(resolved, Some(SecretAuth::ApiKey { .. })));
    }

    #[test]
    fn host_secret_auth_resolution_detects_basic() {
        let _guard = env_lock().lock().expect("env lock");
        let _tmp = setup_env();
        upsert_secret_auth("basic-secret", SecretAuth::basic("elastic", "secret-password"), "pw")
            .expect("save basic secret");

        let resolved = resolve_host_secret_auth(Some("basic-secret")).expect("resolve auth");
        assert!(matches!(resolved, Some(SecretAuth::Basic { .. })));
    }

    #[test]
    fn host_secret_auth_resolution_reads_named_secret() {
        let _guard = env_lock().lock().expect("env lock");
        let _tmp = setup_env();
        upsert_secret_auth("host-fallback", SecretAuth::apikey("secret-key"), "pw").expect("save fallback secret");

        let resolved = resolve_host_secret_auth(Some("host-fallback")).expect("resolve auth");
        assert!(matches!(resolved, Some(SecretAuth::ApiKey { .. })));
    }

    #[cfg(feature = "server")]
    #[test]
    fn serve_runtime_mode_prefers_explicit_flag_over_env() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("ESDIAG_MODE", "service");
        }

        let resolved = resolve_serve_runtime_mode(Some(RuntimeMode::User)).expect("resolve mode");

        assert_eq!(resolved, RuntimeMode::User);

        unsafe {
            std::env::remove_var("ESDIAG_MODE");
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn serve_runtime_mode_uses_env_when_flag_missing() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("ESDIAG_MODE", "service");
        }

        let resolved = resolve_serve_runtime_mode(None).expect("resolve mode");

        assert_eq!(resolved, RuntimeMode::Service);

        unsafe {
            std::env::remove_var("ESDIAG_MODE");
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn serve_runtime_mode_defaults_to_user_without_flag_or_env() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::remove_var("ESDIAG_MODE");
        }

        let resolved = resolve_serve_runtime_mode(None).expect("resolve mode");

        assert_eq!(resolved, RuntimeMode::User);
    }

    #[cfg(feature = "server")]
    #[test]
    fn serve_parses_web_features_flag() {
        let cli = Cli::parse_from(["esdiag", "serve", "--web-features", "advanced,job-builder"]);

        match cli.command {
            Some(Commands::Serve { web_features, .. }) => {
                assert_eq!(web_features.as_deref(), Some("advanced,job-builder"));
            }
            other => panic!("expected serve command, got {other:?}"),
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn serve_exporter_requires_configuration_when_output_is_omitted() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::remove_var("ESDIAG_OUTPUT_URL");
            std::env::remove_var("ESDIAG_OUTPUT_APIKEY");
            std::env::remove_var("ESDIAG_OUTPUT_USERNAME");
            std::env::remove_var("ESDIAG_OUTPUT_PASSWORD");
        }

        let err = match resolve_serve_exporter(None) {
            Ok(_) => panic!("omitted output without a configured deployment must fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("No output deployment is configured"));
    }

    #[cfg(feature = "server")]
    #[test]
    fn serve_exporter_explicit_output_precedes_runtime_environment() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::remove_var("ESDIAG_OUTPUT_URL");
            std::env::set_var("ESDIAG_OUTPUT_APIKEY", "runtime-secret");
            std::env::remove_var("ESDIAG_OUTPUT_USERNAME");
            std::env::remove_var("ESDIAG_OUTPUT_PASSWORD");
        }

        let exporter = resolve_serve_exporter(Some("-".to_string())).expect("resolve explicit output");

        assert_eq!(exporter.target_uri(), "stdio://stdout");

        unsafe {
            std::env::remove_var("ESDIAG_OUTPUT_APIKEY");
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn serve_exporter_uses_runtime_environment_when_output_is_omitted() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("ESDIAG_OUTPUT_URL", "http://localhost:9200");
            std::env::set_var("ESDIAG_OUTPUT_APIKEY", "runtime-secret");
            std::env::remove_var("ESDIAG_OUTPUT_USERNAME");
            std::env::remove_var("ESDIAG_OUTPUT_PASSWORD");
        }

        let exporter = resolve_serve_exporter(None).expect("resolve runtime output");

        assert_eq!(exporter.target_uri(), "http://localhost:9200/");

        unsafe {
            std::env::remove_var("ESDIAG_OUTPUT_URL");
            std::env::remove_var("ESDIAG_OUTPUT_APIKEY");
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn serve_exporter_rejects_partial_runtime_environment_without_leaking_secrets() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::remove_var("ESDIAG_OUTPUT_URL");
            std::env::set_var("ESDIAG_OUTPUT_APIKEY", "do-not-print-this-secret");
            std::env::remove_var("ESDIAG_OUTPUT_USERNAME");
            std::env::remove_var("ESDIAG_OUTPUT_PASSWORD");
        }

        let err = match resolve_serve_exporter(None) {
            Ok(_) => panic!("partial output must fail closed"),
            Err(err) => err,
        };
        let message = err.to_string();
        assert!(message.contains("ESDIAG_OUTPUT_URL is not defined"));
        assert!(!message.contains("do-not-print-this-secret"));

        unsafe {
            std::env::remove_var("ESDIAG_OUTPUT_APIKEY");
        }
    }
}

#[cfg(all(test, feature = "server", feature = "desktop"))]
mod desktop_startup_tests {
    use super::*;
    use std::net::TcpListener;

    #[tokio::test]
    async fn embedded_server_starts_and_serves_local_url() {
        let exporter = Exporter::default();
        let kibana_url = String::new();
        let (mut server, bound_addr) = Server::start([127, 0, 0, 1], 0, exporter, kibana_url, RuntimeMode::User)
            .await
            .expect("desktop embedded server should start");

        let url = format!("http://localhost:{}", bound_addr.port());
        let parsed = tauri::Url::parse(&url).expect("desktop URL should be valid");

        let response = reqwest::get(parsed.as_str())
            .await
            .expect("embedded server should accept HTTP requests");
        assert!(
            response.status().is_success(),
            "expected success status from embedded server, got {}",
            response.status()
        );

        server.shutdown().await;
    }

    #[tokio::test]
    async fn embedded_server_avoids_occupied_port_by_using_ephemeral_binding() {
        let occupied_listener = TcpListener::bind("127.0.0.1:0").expect("should reserve a local test port");
        let occupied_port = occupied_listener
            .local_addr()
            .expect("reserved listener has local addr")
            .port();

        let exporter = Exporter::default();
        let kibana_url = String::new();
        let (mut server, bound_addr) = Server::start([127, 0, 0, 1], 0, exporter, kibana_url, RuntimeMode::User)
            .await
            .expect("desktop embedded server should start while another port is occupied");

        assert_ne!(
            bound_addr.port(),
            occupied_port,
            "ephemeral bind should avoid occupied ports"
        );

        let url = format!("http://localhost:{}", bound_addr.port());
        let response = reqwest::get(&url)
            .await
            .expect("embedded server should accept HTTP requests");
        assert!(
            response.status().is_success(),
            "expected success status from embedded server, got {}",
            response.status()
        );

        server.shutdown().await;
        drop(occupied_listener);
    }
}
