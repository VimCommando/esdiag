use eyre::{Result, WrapErr, eyre};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use esdiag::cli_output::CliOutcome;
#[cfg(feature = "setup")]
use esdiag::{
    client::Client,
    data::{Application, KnownHostBuilder, Uri},
    setup,
};
#[cfg(feature = "setup")]
use url::Url;

const STATE_SCHEMA_VERSION: &str = "3";
const ELASTIC_VERSION: &str = "9.4.2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StackMode {
    Auto,
    Core,
    Full,
}

impl StackMode {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "auto" => Ok(Self::Auto),
            "core" => Ok(Self::Core),
            "full" => Ok(Self::Full),
            _ => Err(eyre!("Stack mode must be auto, full, or core")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Core => "core",
            Self::Full => "full",
        }
    }
}

pub async fn run(args: &[OsString]) -> Result<CliOutcome> {
    let command = args.first().and_then(|argument| argument.to_str()).unwrap_or("help");
    let options = LocalOptions::parse(args.get(1..).unwrap_or_default())?;
    let mut state = LocalState::load(options.state_dir.clone())?;
    state.open_browser = options.open_browser;
    state.copy_password = options.copy_password;

    match command {
        "up" => state.up(options).await?,
        "down" => state.down()?,
        "restart" => {
            if let Some(level) = options.log_level {
                state.values.insert("LOG_LEVEL".to_string(), level);
                state.write()?;
            }
            state.restart(options.remaining)?;
        }
        "status" => state.status()?,
        "logs" => state.logs(options.remaining)?,
        "setup" => {
            state.runtime = Some(detect_runtime(options.runtime)?);
            state.validate_state_coupling()?;
            state.setup().await?;
        }
        "open" => state.open_browser()?,
        "auth" => state.auth().await?,
        "reset" => state.reset(options.force)?,
        "secrets" => state.secret(options.remaining.first().map(String::as_str))?,
        "update" => {
            return Err(eyre!(
                "`esdiag local update` is unavailable. Update the ESDiag binary through its installation channel."
            ));
        }
        "version" | "--version" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            return Ok(empty_outcome(command));
        }
        "help" | "--help" | "-h" => {
            println!("Use `esdiag local <up|down|restart|status|logs|setup|open|auth|secrets|reset>`.");
            return Ok(empty_outcome(command));
        }
        _ => return Err(eyre!("Unknown local command: {command}")),
    }

    Ok(state.outcome(command))
}

fn empty_outcome(command: &str) -> CliOutcome {
    CliOutcome::LocalStack {
        command: command.to_string(),
        mode: None,
        native_service: None,
        esdiag_url: None,
        kibana_url: None,
    }
}

struct LocalOptions {
    state_dir: PathBuf,
    runtime: Option<String>,
    stack: StackMode,
    open_browser: bool,
    copy_password: bool,
    log_level: Option<String>,
    force: bool,
    remaining: Vec<String>,
}

impl LocalOptions {
    fn parse(args: &[OsString]) -> Result<Self> {
        let mut options = Self {
            state_dir: default_state_dir()?,
            runtime: None,
            stack: StackMode::Auto,
            open_browser: true,
            copy_password: true,
            log_level: None,
            force: false,
            remaining: Vec::new(),
        };
        let mut index = 0;
        while index < args.len() {
            let argument = args[index].to_string_lossy();
            match argument.as_ref() {
                "--state-dir" => {
                    index += 1;
                    options.state_dir =
                        PathBuf::from(args.get(index).ok_or_else(|| eyre!("Missing --state-dir value"))?);
                }
                "--runtime" => {
                    index += 1;
                    options.runtime = Some(
                        args.get(index)
                            .ok_or_else(|| eyre!("Missing --runtime value"))?
                            .to_string_lossy()
                            .to_string(),
                    );
                }
                "--stack" => {
                    index += 1;
                    options.stack = StackMode::parse(
                        &args
                            .get(index)
                            .ok_or_else(|| eyre!("Missing --stack value"))?
                            .to_string_lossy(),
                    )?;
                }
                "--open-browser=false" => options.open_browser = false,
                "--open-browser=true" => options.open_browser = true,
                "--copy-password=false" => options.copy_password = false,
                "--copy-password=true" => options.copy_password = true,
                "--log-level" => {
                    index += 1;
                    options.log_level = Some(
                        args.get(index)
                            .ok_or_else(|| eyre!("Missing --log-level value"))?
                            .to_string_lossy()
                            .to_string(),
                    );
                }
                "--force" => options.force = true,
                value if value.starts_with("--state-dir=") => {
                    options.state_dir = PathBuf::from(&value["--state-dir=".len()..])
                }
                value if value.starts_with("--runtime=") => {
                    options.runtime = Some(value["--runtime=".len()..].to_string())
                }
                value if value.starts_with("--stack=") => options.stack = StackMode::parse(&value["--stack=".len()..])?,
                value if value.starts_with("--log-level=") => {
                    options.log_level = Some(value["--log-level=".len()..].to_string())
                }
                value if value.starts_with('-') => return Err(eyre!("Unsupported native local option: {value}")),
                value => options.remaining.push(value.to_string()),
            }
            index += 1;
        }
        Ok(options)
    }
}

struct LocalState {
    dir: PathBuf,
    values: BTreeMap<String, String>,
    runtime: Option<String>,
    open_browser: bool,
    copy_password: bool,
    has_existing_state: bool,
}

impl LocalState {
    fn load(dir: PathBuf) -> Result<Self> {
        let env_path = dir.join(".env");
        let has_existing_state = env_path.exists();
        let values = match fs::read_to_string(&env_path) {
            Ok(contents) => parse_env(&contents)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error).wrap_err("failed to read local stack state"),
        };
        Ok(Self {
            dir,
            values,
            runtime: None,
            open_browser: true,
            copy_password: true,
            has_existing_state,
        })
    }

    async fn up(&mut self, options: LocalOptions) -> Result<()> {
        self.up_inner(options).await.map_err(|error| {
            let message = format!(
                "Local startup did not complete: {error}. Generated state is retained at {}. Inspect `esdiag local logs --state-dir {}`, then retry `esdiag local up --state-dir {}` or stop with `esdiag local down --state-dir {}`.",
                self.dir.display(), self.dir.display(), self.dir.display(), self.dir.display()
            );
            error.wrap_err(message)
        })
    }

    async fn up_inner(&mut self, options: LocalOptions) -> Result<()> {
        self.runtime = Some(detect_runtime(options.runtime)?);
        self.open_browser = options.open_browser;
        self.copy_password = options.copy_password;
        self.validate_state_coupling()?;
        fs::create_dir_all(self.dir.join("logs"))?;
        secure_dir(&self.dir)?;
        let mode = self.resolve_mode(options.stack)?;
        let previous_mode = self.has_existing_state.then(|| self.active_mode());
        self.initialize(mode)?;
        if let Some(level) = options.log_level {
            self.values.insert("LOG_LEVEL".to_string(), level);
        }
        self.write()?;
        if previous_mode == Some(StackMode::Full) && mode == StackMode::Core {
            self.compose(&["down", "--remove-orphans"])?;
        }
        self.compose(&["pull", "elasticsearch", "kibana"])?;
        self.compose(&["up", "-d", "elasticsearch", "kibana"])?;
        self.wait_elasticsearch().await?;
        self.configure_security().await?;
        self.write()?;
        self.wait_kibana().await?;
        self.setup_mode(mode).await?;
        if mode == StackMode::Full {
            self.stop_native_service()?;
            self.compose(&["up", "-d", "esdiag"])?;
        } else {
            self.start_native_service()?;
        }
        self.wait_url(&self.esdiag_url(), None).await?;
        self.values.insert("STACK_MODE".to_string(), mode.as_str().to_string());
        self.write()?;
        if self.open_browser {
            self.open_browser()?;
        }
        Ok(())
    }

    fn resolve_mode(&self, requested: StackMode) -> Result<StackMode> {
        match requested {
            StackMode::Auto => match self.values.get("STACK_MODE").map(String::as_str) {
                Some("full") => Ok(StackMode::Full),
                Some("core") => Ok(StackMode::Core),
                Some(other) => Err(eyre!("Unsupported local stack mode: {other}")),
                None if self.has_existing_state => Ok(StackMode::Full),
                None => Ok(StackMode::Core),
            },
            mode => Ok(mode),
        }
    }

    fn active_mode(&self) -> StackMode {
        self.values
            .get("STACK_MODE")
            .and_then(|value| StackMode::parse(value).ok())
            .unwrap_or({
                if self.has_existing_state {
                    StackMode::Full
                } else {
                    StackMode::Core
                }
            })
    }

    fn validate_state_coupling(&self) -> Result<()> {
        let runtime = self
            .runtime
            .as_deref()
            .expect("runtime initialized before state validation");
        let project = format!("esdiag-local-{}", stable_project_id(&self.dir));
        let volume_exists = |volume: &str| {
            Command::new(runtime)
                .args(["volume", "inspect", &format!("{project}_{volume}")])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        };
        let elasticsearch_volume = volume_exists("elasticsearch-data");
        let kibana_volume = volume_exists("kibana-data");
        let esdiag_volume = volume_exists("esdiag-data");
        if !self.has_existing_state {
            if elasticsearch_volume || kibana_volume || esdiag_volume {
                return Err(eyre!(
                    "Deployment volumes exist but {} is missing; restore it or run `esdiag local reset --force`",
                    self.dir.join(".env").display()
                ));
            }
            return Ok(());
        }
        if self
            .values
            .get("ESDIAG_OUTPUT_APIKEY")
            .is_some_and(|key| key != "pending")
            && (!elasticsearch_volume || !kibana_volume)
        {
            return Err(eyre!(
                "Credential state exists but deployment volumes are missing; restore them or run `esdiag local reset --force`"
            ));
        }
        Ok(())
    }

    fn initialize(&mut self, mode: StackMode) -> Result<()> {
        let version = env!("CARGO_PKG_VERSION");
        self.value("STATE_SCHEMA_VERSION", STATE_SCHEMA_VERSION);
        self.value("STACK_ESDIAG_VERSION", version);
        self.value("STACK_ELASTIC_VERSION", ELASTIC_VERSION);
        self.value("ELASTIC_SECURITY_ENABLED", "true");
        self.value("ELASTIC_PASSWORD", &random_secret());
        self.value("KIBANA_SYSTEM_PASSWORD", &random_secret());
        self.value("KIBANA_ENCRYPTION_KEY", &random_secret());
        self.value("ESDIAG_OUTPUT_APIKEY", "pending");
        self.value(
            "ELASTICSEARCH_IMAGE",
            &format!("docker.elastic.co/elasticsearch/elasticsearch:{ELASTIC_VERSION}"),
        );
        self.value(
            "KIBANA_IMAGE",
            &format!("docker.elastic.co/kibana/kibana:{ELASTIC_VERSION}"),
        );
        self.value("ESDIAG_IMAGE", &format!("docker.elastic.co/esdiag/esdiag:{version}"));
        self.value("ESDIAG_ELASTICSEARCH_PORT", "9200");
        self.value("ESDIAG_KIBANA_PORT", "5601");
        self.value("ESDIAG_PORT", "2501");
        self.value("ESDIAG_KIBANA_SPACE", "esdiag");
        let kibana_port = self.required("ESDIAG_KIBANA_PORT")?;
        let kibana_space = self.required("ESDIAG_KIBANA_SPACE")?;
        self.value(
            "ESDIAG_KIBANA_PUBLIC_URL",
            &esdiag::env::kibana_url_with_space(&format!("http://127.0.0.1:{kibana_port}"), Some(kibana_space)),
        );
        self.value("LOG_LEVEL", "info");
        self.write_compose(mode)
    }

    fn value(&mut self, key: &str, default: &str) {
        self.values
            .entry(key.to_string())
            .or_insert_with(|| default.to_string());
    }

    fn write(&self) -> Result<()> {
        let env = self
            .values
            .iter()
            .map(|(key, value)| format!("{key}={value}\n"))
            .collect::<String>();
        write_private(self.dir.join(".env"), &env)?;
        Ok(())
    }

    fn write_compose(&self, mode: StackMode) -> Result<()> {
        let full = mode == StackMode::Full;
        let compose = format!(
            "name: esdiag-local\nservices:\n  elasticsearch:\n    image: ${{ELASTICSEARCH_IMAGE}}\n    environment:\n      discovery.type: single-node\n      xpack.security.enabled: \"true\"\n      xpack.security.http.ssl.enabled: \"false\"\n      ELASTIC_PASSWORD: ${{ELASTIC_PASSWORD}}\n    ports: [\"127.0.0.1:${{ESDIAG_ELASTICSEARCH_PORT}}:9200\"]\n    volumes: [\"elasticsearch-data:/usr/share/elasticsearch/data\"]\n  kibana:\n    image: ${{KIBANA_IMAGE}}\n    environment:\n      ELASTICSEARCH_HOSTS: http://elasticsearch:9200\n      ELASTICSEARCH_USERNAME: kibana_system\n      ELASTICSEARCH_PASSWORD: ${{KIBANA_SYSTEM_PASSWORD}}\n      XPACK_ENCRYPTEDSAVEDOBJECTS_ENCRYPTIONKEY: ${{KIBANA_ENCRYPTION_KEY}}\n    ports: [\"127.0.0.1:${{ESDIAG_KIBANA_PORT}}:5601\"]\n    volumes: [\"kibana-data:/usr/share/kibana/data\"]\n{full_services}volumes:\n  elasticsearch-data:\n  kibana-data:\n{full_volume}",
            full_services = if full {
                "  setup:\n    image: ${ESDIAG_IMAGE}\n    profiles: [\"setup\"]\n    environment:\n      ESDIAG_OUTPUT_URL: http://elasticsearch:9200\n      ESDIAG_OUTPUT_APIKEY: ${ESDIAG_OUTPUT_APIKEY}\n      ESDIAG_KIBANA_SPACE: ${ESDIAG_KIBANA_SPACE}\n      ESDIAG_KIBANA_URL: http://kibana:5601\n    command: [\"setup\"]\n  esdiag:\n    image: ${ESDIAG_IMAGE}\n    environment:\n      ESDIAG_MODE: user\n      ESDIAG_CONTAINER_LOCAL_STACK: full\n      ESDIAG_OUTPUT_URL: http://elasticsearch:9200\n      ESDIAG_OUTPUT_APIKEY: ${ESDIAG_OUTPUT_APIKEY}\n      ESDIAG_KIBANA_SPACE: ${ESDIAG_KIBANA_SPACE}\n      ESDIAG_KIBANA_URL: http://kibana:5601\n      ESDIAG_KIBANA_INTERNAL_URL: http://kibana:5601\n      ESDIAG_KIBANA_PUBLIC_URL: ${ESDIAG_KIBANA_PUBLIC_URL}\n    command: [\"serve\"]\n    ports: [\"127.0.0.1:${ESDIAG_PORT}:2501\"]\n    volumes: [\"esdiag-data:/root/.esdiag\"]\n"
            } else {
                ""
            },
            full_volume = if full { "  esdiag-data:\n" } else { "" }
        );
        write_private(self.dir.join("compose.yml"), &compose)
    }

    async fn configure_security(&mut self) -> Result<()> {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build()?;
        let response = client
            .post(format!(
                "{}/_security/user/kibana_system/_password",
                self.elasticsearch_url()
            ))
            .basic_auth("elastic", Some(self.required("ELASTIC_PASSWORD")?))
            .json(&serde_json::json!({"password": self.required("KIBANA_SYSTEM_PASSWORD")?}))
            .send()
            .await?
            .error_for_status()?;
        drop(response);
        if self.required("ESDIAG_OUTPUT_APIKEY")? == "pending" {
            let response: serde_json::Value = client
                .post(format!("{}/_security/api_key", self.elasticsearch_url()))
                .basic_auth("elastic", Some(self.required("ELASTIC_PASSWORD")?))
                .json(&serde_json::json!({"name": "esdiag-local"}))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let key = response["encoded"]
                .as_str()
                .ok_or_else(|| eyre!("Elasticsearch did not return an API key"))?;
            self.values.insert("ESDIAG_OUTPUT_APIKEY".to_string(), key.to_string());
        }
        Ok(())
    }

    async fn setup(&mut self) -> Result<()> {
        self.setup_mode(self.active_mode()).await
    }

    async fn setup_mode(&mut self, mode: StackMode) -> Result<()> {
        if mode == StackMode::Full {
            self.compose(&["--profile", "setup", "run", "--rm", "--no-deps", "setup"])
        } else {
            self.native_setup().await
        }
    }

    #[cfg(not(feature = "setup"))]
    async fn native_setup(&self) -> Result<()> {
        Err(eyre!(
            "Native local-stack setup requires an ESDiag binary built with the setup feature."
        ))
    }

    #[cfg(feature = "setup")]
    async fn native_setup(&self) -> Result<()> {
        let apikey = self.required("ESDIAG_OUTPUT_APIKEY")?.to_string();
        let elasticsearch = KnownHostBuilder::new(Url::parse(&self.elasticsearch_url())?)
            .application(Application::Elasticsearch)
            .apikey(Some(apikey.clone()))
            .build()?;
        let elasticsearch = Client::try_from(Uri::KnownHost(elasticsearch))?;
        setup::assets(&elasticsearch).await?;
        setup::ensure_agent_builder_license(&elasticsearch).await?;

        let kibana = KnownHostBuilder::new(Url::parse(&self.kibana_url())?)
            .application(Application::Kibana)
            .apikey(Some(apikey))
            .build()?;
        let kibana = Client::try_from(Uri::KnownHost(kibana))?;
        setup::assets(&kibana).await
    }

    fn start_native_service(&self) -> Result<()> {
        self.stop_native_service()?;
        let log = self.dir.join("logs/native-serve.log");
        let stdout = fs::File::create(&log)?;
        let child = Command::new(std::env::current_exe()?)
            .arg("serve")
            .arg("--mode")
            .arg("user")
            .arg("--bind")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(self.required("ESDIAG_PORT")?)
            .env("ESDIAG_OUTPUT_URL", self.elasticsearch_url())
            .env("ESDIAG_OUTPUT_APIKEY", self.required("ESDIAG_OUTPUT_APIKEY")?)
            .env("ESDIAG_KIBANA_URL", self.kibana_url())
            .env("LOG_LEVEL", self.required("LOG_LEVEL")?)
            .stdout(Stdio::from(stdout.try_clone()?))
            .stderr(Stdio::from(stdout))
            .spawn()?;
        write_private(self.dir.join(".native-serve.pid"), &child.id().to_string())?;
        write_private(
            self.dir.join(".native-serve.binary"),
            &std::env::current_exe()?.to_string_lossy(),
        )?;
        write_private(
            self.dir.join(".native-serve.started"),
            &process_start_time(child.id() as i32)?
                .ok_or_else(|| eyre!("Could not identify managed native ESDiag service"))?,
        )?;
        Ok(())
    }

    fn stop_native_service(&self) -> Result<()> {
        let path = self.dir.join(".native-serve.pid");
        let binary_path = self.dir.join(".native-serve.binary");
        let started_path = self.dir.join(".native-serve.started");
        let Ok(pid) = fs::read_to_string(&path) else {
            return Ok(());
        };
        let pid = pid
            .trim()
            .parse::<i32>()
            .map_err(|_| eyre!("Invalid managed native service PID"))?;
        let binary = fs::read_to_string(&binary_path).unwrap_or_default();
        let started = fs::read_to_string(&started_path).unwrap_or_default();
        #[cfg(unix)]
        {
            let command = Command::new("ps")
                .args(["-p", &pid.to_string(), "-o", "command="])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).to_string());
            if !binary.trim().is_empty()
                && !started.trim().is_empty()
                && command.is_some_and(|command| command.contains(binary.trim()) && command.contains("serve"))
                && process_start_time(pid)?.as_deref() == Some(started.trim())
            {
                let status = Command::new("kill").args(["-TERM", &pid.to_string()]).status()?;
                if !status.success() {
                    return Err(eyre!("Could not stop the managed native ESDiag service"));
                }
            }
        }
        fs::remove_file(path)?;
        let _ = fs::remove_file(binary_path);
        let _ = fs::remove_file(started_path);
        Ok(())
    }

    fn native_service_state(&self) -> &'static str {
        let pid = fs::read_to_string(self.dir.join(".native-serve.pid"))
            .ok()
            .and_then(|value| value.trim().parse::<i32>().ok());
        let binary = fs::read_to_string(self.dir.join(".native-serve.binary")).unwrap_or_default();
        let started = fs::read_to_string(self.dir.join(".native-serve.started")).unwrap_or_default();
        let Some(pid) = pid else {
            return "stopped";
        };
        let command = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "command="])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string());
        if !binary.trim().is_empty()
            && !started.trim().is_empty()
            && command.is_some_and(|command| command.contains(binary.trim()) && command.contains("serve"))
            && process_start_time(pid).ok().flatten().as_deref() == Some(started.trim())
        {
            "running"
        } else {
            "stale"
        }
    }

    fn down(&mut self) -> Result<()> {
        self.runtime = Some(detect_runtime(None)?);
        self.stop_native_service()?;
        self.compose(&["down"])
    }

    fn restart(&mut self, services: Vec<String>) -> Result<()> {
        self.runtime = Some(detect_runtime(None)?);
        for service in services {
            if service == "esdiag" && self.values.get("STACK_MODE").map(String::as_str) == Some("core") {
                self.start_native_service()?;
            } else {
                self.compose(&["up", "-d", "--no-deps", "--force-recreate", &service])?;
            }
        }
        Ok(())
    }

    fn status(&mut self) -> Result<()> {
        self.runtime = Some(detect_runtime(None)?);
        self.compose(&["ps"])
    }

    fn logs(&mut self, services: Vec<String>) -> Result<()> {
        self.runtime = Some(detect_runtime(None)?);
        if self.values.get("STACK_MODE").map(String::as_str) == Some("core")
            && (services.is_empty() || services.first().map(String::as_str) == Some("esdiag"))
        {
            eprint!(
                "{}",
                fs::read_to_string(self.dir.join("logs/native-serve.log")).unwrap_or_default()
            );
            Ok(())
        } else {
            let refs = services.iter().map(String::as_str).collect::<Vec<_>>();
            let mut args = vec!["logs"];
            args.extend(refs);
            self.compose(&args)
        }
    }

    async fn auth(&self) -> Result<()> {
        self.wait_elasticsearch().await?;
        self.wait_kibana().await
    }

    fn reset(&mut self, force: bool) -> Result<()> {
        if !force {
            return Err(eyre!("reset requires --force"));
        }
        self.runtime = Some(detect_runtime(None)?);
        self.stop_native_service()?;
        self.compose(&["down", "--volumes", "--remove-orphans"])?;
        fs::remove_dir_all(&self.dir)?;
        Ok(())
    }

    fn secret(&self, kind: Option<&str>) -> Result<()> {
        let key = match kind {
            Some("password") => "ELASTIC_PASSWORD",
            Some("apikey") => "ESDIAG_OUTPUT_APIKEY",
            _ => return Err(eyre!("Usage: esdiag local secrets password|apikey")),
        };
        println!("{}", self.required(key)?);
        Ok(())
    }

    fn open_browser(&self) -> Result<()> {
        if self.copy_password {
            self.copy_password_to_clipboard();
        }
        #[cfg(target_os = "macos")]
        let opener = Command::new("open")
            .arg(self.esdiag_url())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status();
        #[cfg(target_os = "linux")]
        let opener = Command::new("xdg-open")
            .arg(self.esdiag_url())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status();
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if let Err(error) = opener {
            eprintln!(
                "Could not open the browser: {error}. Open {} manually.",
                self.esdiag_url()
            );
        }
        Ok(())
    }

    fn copy_password_to_clipboard(&self) {
        let password = match self.required("ELASTIC_PASSWORD") {
            Ok(password) => password,
            Err(error) => {
                eprintln!("Could not retrieve the Kibana password for the clipboard: {error}");
                return;
            }
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        return;
        #[cfg(target_os = "macos")]
        let copied = write_clipboard("pbcopy", &[], password);
        #[cfg(target_os = "linux")]
        let copied = [
            ("wl-copy", Vec::new()),
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
        ]
        .into_iter()
        .any(|(command, args)| write_clipboard(command, &args, password));
        if copied {
            eprintln!("Copied the elastic password to the clipboard");
        } else {
            eprintln!("Could not copy the elastic password; use `esdiag local secrets password`.");
        }
    }

    fn compose(&self, arguments: &[&str]) -> Result<()> {
        let runtime = self
            .runtime
            .as_deref()
            .ok_or_else(|| eyre!("Container runtime is not initialized"))?;
        let project = format!("esdiag-local-{}", stable_project_id(&self.dir));
        let status = Command::new(runtime)
            .args(["compose", "--project-name", &project, "--env-file"])
            .arg(self.dir.join(".env"))
            .args(["--file"])
            .arg(self.dir.join("compose.yml"))
            .args(arguments)
            .stdin(Stdio::inherit())
            .stdout(Stdio::from(std::io::stderr()))
            .stderr(Stdio::inherit())
            .status()?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| eyre!("{runtime} compose command failed"))
    }

    async fn wait_elasticsearch(&self) -> Result<()> {
        self.wait_url(
            &self.elasticsearch_url(),
            Some(("elastic", self.required("ELASTIC_PASSWORD")?)),
        )
        .await
    }

    async fn wait_kibana(&self) -> Result<()> {
        let url = format!("{}/api/status", self.kibana_url());
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build()?;
        for _ in 0..60 {
            if let Ok(response) = client.get(&url).send().await
                && response.status().is_success()
                && response
                    .json::<serde_json::Value>()
                    .await
                    .is_ok_and(|response| kibana_is_available(&response))
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        Err(eyre!("Timed out waiting for Kibana to become available at {url}"))
    }

    async fn wait_url(&self, url: &str, auth: Option<(&str, &str)>) -> Result<()> {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build()?;
        for _ in 0..60 {
            let request = client.get(url);
            let request = match auth {
                Some((username, password)) => request.basic_auth(username, Some(password)),
                None => request,
            };
            if let Ok(response) = request.send().await {
                let status = response.status();
                if local_endpoint_ready(status, auth.is_some()) {
                    return Ok(());
                }
                if auth.is_some() && matches!(status.as_u16(), 401 | 403) {
                    return Err(eyre!(
                        "Authentication failed at {url} (HTTP {status}); check the retained local credentials."
                    ));
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        Err(eyre!("Timed out waiting for {url}"))
    }

    fn required(&self, key: &str) -> Result<&str> {
        self.values
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| eyre!("Missing {key} in local state"))
    }

    fn elasticsearch_url(&self) -> String {
        format!(
            "http://127.0.0.1:{}",
            self.required("ESDIAG_ELASTICSEARCH_PORT").unwrap_or("9200")
        )
    }
    fn kibana_url(&self) -> String {
        esdiag::env::kibana_url_with_space(
            &format!(
                "http://127.0.0.1:{}",
                self.required("ESDIAG_KIBANA_PORT").unwrap_or("5601")
            ),
            Some(self.required("ESDIAG_KIBANA_SPACE").unwrap_or("esdiag")),
        )
    }
    fn esdiag_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.required("ESDIAG_PORT").unwrap_or("2501"))
    }

    fn outcome(&self, command: &str) -> CliOutcome {
        CliOutcome::LocalStack {
            command: command.to_string(),
            mode: (!self.values.is_empty() || self.has_existing_state).then(|| self.active_mode().as_str().to_string()),
            native_service: (self.active_mode() == StackMode::Core).then(|| self.native_service_state().to_string()),
            esdiag_url: Some(self.esdiag_url()),
            kibana_url: Some(self.kibana_url()),
        }
    }
}

fn write_clipboard(command: &str, arguments: &[&str], value: &str) -> bool {
    Command::new(command)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .expect("piped clipboard stdin")
                .write_all(value.as_bytes())?;
            child.wait()
        })
        .is_ok_and(|status| status.success())
}

fn process_start_time(pid: i32) -> Result<Option<String>> {
    #[cfg(unix)]
    {
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "lstart="])
            .output()?;
        Ok(output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|started| !started.is_empty()))
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        Ok(None)
    }
}

fn default_state_dir() -> Result<PathBuf> {
    let home = std::env::var_os("ESDIAG_LOCAL_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".esdiag/local")))
        .ok_or_else(|| eyre!("Cannot determine the local stack state directory"))?;
    Ok(home)
}

fn detect_runtime(requested: Option<String>) -> Result<String> {
    for runtime in requested
        .into_iter()
        .chain(["podman".to_string(), "docker".to_string()])
    {
        if Command::new(&runtime)
            .arg("compose")
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return Ok(runtime);
        }
    }
    Err(eyre!("Podman or Docker with Compose support is required"))
}

pub fn detected_runtime() -> Option<String> {
    detect_runtime(None).ok()
}

fn parse_env(contents: &str) -> Result<BTreeMap<String, String>> {
    contents
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split_once('=').ok_or_else(|| eyre!("Invalid local state line")))
        .map(|entry| entry.map(|(key, value)| (key.to_string(), value.to_string())))
        .collect()
}

fn kibana_is_available(response: &serde_json::Value) -> bool {
    response
        .pointer("/status/overall/level")
        .and_then(serde_json::Value::as_str)
        == Some("available")
}

fn random_secret() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
fn stable_project_id(path: &Path) -> String {
    posix_cksum(path.to_string_lossy().as_bytes()).to_string()
}

fn posix_cksum(bytes: &[u8]) -> u32 {
    let mut checksum = 0_u32;
    for byte in bytes.iter().copied().chain(
        std::iter::successors((!bytes.is_empty()).then_some(bytes.len()), |length| {
            (*length > 0xff).then_some(*length >> 8)
        })
        .map(|length| length as u8),
    ) {
        checksum ^= u32::from(byte) << 24;
        for _ in 0..8 {
            checksum = if checksum & 0x8000_0000 != 0 {
                (checksum << 1) ^ 0x04c1_1db7
            } else {
                checksum << 1
            };
        }
    }
    !checksum
}

fn write_private(path: PathBuf, contents: &str) -> Result<()> {
    fs::write(&path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn secure_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{LocalState, StackMode, kibana_is_available, stable_project_id};
    use esdiag::cli_output::CliOutcome;
    use std::{collections::BTreeMap, fs, path::Path};
    use tempfile::TempDir;

    fn state() -> (TempDir, LocalState) {
        let directory = TempDir::new().expect("temporary local state");
        let state = LocalState {
            dir: directory.path().to_path_buf(),
            values: BTreeMap::new(),
            runtime: None,
            open_browser: false,
            copy_password: false,
            has_existing_state: false,
        };
        (directory, state)
    }

    #[test]
    fn native_auto_defaults_to_core() {
        let (_directory, state) = state();

        assert_eq!(
            state.resolve_mode(StackMode::Auto).expect("resolve mode"),
            StackMode::Core
        );
    }

    #[test]
    fn default_space_local_urls_and_containers_use_explicit_selection() {
        let (directory, mut state) = state();
        state.values.insert("ESDIAG_KIBANA_SPACE".into(), "_default".into());
        assert_eq!(state.kibana_url(), "http://127.0.0.1:5601");
        state.write_compose(StackMode::Full).unwrap();
        let compose = fs::read_to_string(directory.path().join("compose.yml")).unwrap();
        assert_eq!(
            compose.matches("ESDIAG_KIBANA_SPACE: ${ESDIAG_KIBANA_SPACE}").count(),
            2
        );
        assert!(!compose.contains("/s/${ESDIAG_KIBANA_SPACE}"));
    }

    #[test]
    fn legacy_state_without_a_mode_remains_full() {
        let (_directory, mut state) = state();
        state.has_existing_state = true;

        assert_eq!(
            state.resolve_mode(StackMode::Auto).expect("resolve legacy mode"),
            StackMode::Full
        );
        assert_eq!(state.active_mode(), StackMode::Full);
    }

    #[test]
    fn compose_generation_keeps_core_and_full_runtime_state_separate() {
        let (_directory, mut state) = state();
        state.initialize(StackMode::Core).expect("initialize core state");
        let core = fs::read_to_string(state.dir.join("compose.yml")).expect("read core compose");
        assert!(!core.contains("\n  esdiag:\n"));
        assert!(!core.contains("esdiag-data"));

        state.initialize(StackMode::Full).expect("initialize full state");
        let full = fs::read_to_string(state.dir.join("compose.yml")).expect("read full compose");
        assert!(full.contains("\n  esdiag:\n"));
        assert!(full.contains("esdiag-data"));
    }

    #[test]
    fn existing_mode_is_retained_by_auto() {
        let (_directory, mut state) = state();
        state.values.insert("STACK_MODE".to_string(), "full".to_string());

        assert_eq!(
            state.resolve_mode(StackMode::Auto).expect("resolve mode"),
            StackMode::Full
        );
    }

    #[test]
    fn core_outcome_reports_a_stopped_managed_native_service() {
        let (_directory, mut state) = state();
        state.values.insert("STACK_MODE".to_string(), "core".to_string());

        let CliOutcome::LocalStack {
            mode, native_service, ..
        } = state.outcome("status")
        else {
            panic!("local lifecycle outcome");
        };
        assert_eq!(mode.as_deref(), Some("core"));
        assert_eq!(native_service.as_deref(), Some("stopped"));
    }

    #[test]
    fn project_name_matches_standalone_cksum_algorithm() {
        assert_eq!(stable_project_id(Path::new("/tmp/esdiag-local")), "4235528030");
    }

    #[test]
    fn kibana_readiness_requires_an_available_overall_status() {
        assert!(!kibana_is_available(&serde_json::json!({
            "status": {"overall": {"level": "degraded"}}
        })));
        assert!(kibana_is_available(&serde_json::json!({
            "status": {"overall": {"level": "available"}}
        })));
    }
}

fn local_endpoint_ready(status: reqwest::StatusCode, authenticated: bool) -> bool {
    status.is_success() || (!authenticated && status == reqwest::StatusCode::UNAUTHORIZED)
}

#[cfg(test)]
mod onboarding_recovery_tests {
    use super::*;

    #[tokio::test]
    async fn readiness_requests_run_inside_the_async_lifecycle() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = [0; 4096];
                socket.read(&mut buffer).await.unwrap();
                socket
                    .write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await
                    .unwrap();
            }
        });
        let dir = tempfile::tempdir().unwrap();
        let state = LocalState::load(dir.path().into()).unwrap();
        assert!(state.wait_url(&url, None).await.is_ok());
        let error = state
            .wait_url(&url, Some(("elastic", "bad-test-password")))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Authentication failed"));
        server.await.unwrap();
    }

    #[test]
    fn service_reachability_does_not_mask_bad_elasticsearch_credentials() {
        assert!(local_endpoint_ready(reqwest::StatusCode::OK, true));
        assert!(local_endpoint_ready(reqwest::StatusCode::UNAUTHORIZED, false));
        assert!(!local_endpoint_ready(reqwest::StatusCode::UNAUTHORIZED, true));
        assert!(!local_endpoint_ready(reqwest::StatusCode::SERVICE_UNAVAILABLE, false));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_startup_retains_recovery_files() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("runtime");
        fs::write(
            &runtime,
            "#!/bin/sh\ncase \"$*\" in\n  'compose version') exit 0;;\n  *) exit 1;;\nesac\n",
        )
        .unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let state_dir = dir.path().join("state");
        let mut state = LocalState::load(state_dir.clone()).unwrap();
        let mut options = LocalOptions::parse(&[]).unwrap();
        options.runtime = Some(runtime.to_string_lossy().into_owned());
        assert!(state.up(options).await.is_err());
        let env = fs::read_to_string(state_dir.join(".env")).unwrap();
        assert!(parse_env(&env).unwrap().contains_key("ELASTIC_PASSWORD"));
        assert!(state_dir.join("compose.yml").exists());
    }
}
