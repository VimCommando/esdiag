// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

//! Flow-neutral operations used by terminal and future GUI onboarding.

use crate::data::{
    Application, ApplicationConfig, HostRole, Job, JobOutput, KnownHost, KnownHostBuilder, SavedJobs, SecretAuth,
    keystore_exists, save_saved_jobs, upsert_secret_auth,
};
use eyre::{Result, eyre};
use url::Url;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OnboardingReadiness {
    pub user_configured: bool,
    pub keystore_ready: bool,
    pub output_configured: bool,
    pub collect_host_configured: bool,
    pub default_job_configured: bool,
}

impl OnboardingReadiness {
    pub fn is_complete(&self) -> bool {
        self.user_configured && self.collect_host_configured && self.default_job_configured
    }
}

#[derive(Clone, Debug)]
pub struct OutputDeploymentInput {
    pub output_name: String,
    pub output_url: Url,
    pub viewer_name: String,
    pub viewer_url: Url,
    pub secret_id: String,
    pub auth: SecretAuth,
}

#[derive(Clone, Debug)]
pub struct CollectHostInput {
    pub name: String,
    pub app: Application,
    pub url: Url,
    pub secret_id: Option<String>,
    pub auth: Option<SecretAuth>,
}

pub fn inspect() -> Result<OnboardingReadiness> {
    let config = ApplicationConfig::load()?;
    let hosts = KnownHost::parse_hosts_yml()?;
    let jobs = crate::data::load_saved_jobs()?;

    let output_configured = config
        .output
        .default
        .as_ref()
        .is_some_and(|name| hosts.get(name).is_some_and(valid_output_host));
    let collect_host_configured = hosts.values().any(|host| host.has_role(HostRole::Collect));
    let default_job_configured = config.job.default.as_ref().is_some_and(|name| jobs.contains_key(name));

    Ok(OnboardingReadiness {
        user_configured: config.user.as_deref().is_some_and(|user| !user.trim().is_empty()),
        keystore_ready: keystore_exists()?,
        output_configured,
        collect_host_configured,
        default_job_configured,
    })
}

pub fn save_user(user: String) -> Result<ApplicationConfig> {
    let user = user.trim();
    if user.is_empty() {
        return Err(eyre!("A default diagnostic user is required"));
    }
    let mut config = ApplicationConfig::load()?;
    config.user = Some(user.to_string());
    config.save()?;
    Ok(config)
}

/// Persists one linked Elasticsearch/Kibana deployment after the caller has
/// completed any remote validation it requires.
pub fn save_output_deployment(input: OutputDeploymentInput, keystore_password: &str) -> Result<ApplicationConfig> {
    validate_name(&input.output_name, "output host")?;
    validate_name(&input.viewer_name, "Kibana viewer")?;
    validate_name(&input.secret_id, "secret")?;
    if input.output_name == input.viewer_name {
        return Err(eyre!("Output host and Kibana viewer names must differ"));
    }

    upsert_secret_auth(&input.secret_id, input.auth.clone(), keystore_password)?;
    let output = KnownHostBuilder::new(input.output_url)
        .application(Application::Elasticsearch)
        .roles(vec![HostRole::Send])
        .viewer(Some(input.viewer_name.clone()))
        .secret(Some(input.secret_id.clone()))
        .build_with_secret_auth(input.auth.clone())?;
    let viewer = KnownHostBuilder::new(input.viewer_url)
        .application(Application::Kibana)
        .roles(vec![HostRole::View])
        .secret(Some(input.secret_id))
        .build_with_secret_auth(input.auth)?;

    let mut hosts = KnownHost::parse_hosts_yml()?;
    hosts.insert(input.output_name.clone(), output);
    hosts.insert(input.viewer_name, viewer);
    KnownHost::write_hosts_yml(&hosts)?;

    let mut config = ApplicationConfig::load()?;
    config.output.default = Some(input.output_name);
    config.save()?;
    Ok(config)
}

pub fn save_collect_host(input: CollectHostInput, keystore_password: Option<&str>) -> Result<()> {
    validate_name(&input.name, "collect host")?;
    match (&input.secret_id, &input.auth, keystore_password) {
        (Some(secret_id), Some(auth), Some(password)) => upsert_secret_auth(secret_id, auth.clone(), password)?,
        (Some(_), None, _) | (None, None, _) => {}
        _ => {
            return Err(eyre!(
                "Collect-host credentials require a secret id, authentication value, and unlocked keystore"
            ));
        }
    }

    let builder = KnownHostBuilder::new(input.url)
        .application(input.app)
        .roles(vec![HostRole::Collect])
        .secret(input.secret_id);
    let host = match input.auth {
        Some(auth) => builder.build_with_secret_auth(auth)?,
        None => builder.build()?,
    };
    let mut hosts = KnownHost::parse_hosts_yml()?;
    if let Some(existing) = hosts.get(&input.name).cloned() {
        hosts.insert(input.name, existing.with_role(HostRole::Collect));
        KnownHost::write_hosts_yml(&hosts)?;
        return Ok(());
    }
    hosts.insert(input.name, host);
    KnownHost::write_hosts_yml(&hosts)?;
    Ok(())
}

pub fn save_default_processing_job(name: String, collect_host: String) -> Result<ApplicationConfig> {
    validate_name(&name, "default job")?;
    let collect = KnownHost::get_known(&collect_host)
        .ok_or_else(|| eyre!("Configured collect host '{collect_host}' was not found"))?;
    if !collect.has_role(HostRole::Collect) {
        return Err(eyre!(
            "Configured collect host '{collect_host}' must include role 'collect'"
        ));
    }
    let config = ApplicationConfig::load()?;
    let output = config
        .output
        .default
        .clone()
        .ok_or_else(|| eyre!("A validated output deployment is required before creating a default job"))?;
    let job = Job::builder()
        .collect_from(collect_host)?
        .process_to(JobOutput::KnownHost { name: output })?;
    save_default_job(name, job)
}

pub fn save_default_job(name: String, job: Job) -> Result<ApplicationConfig> {
    validate_name(&name, "default job")?;
    let mut jobs: SavedJobs = crate::data::load_saved_jobs()?;
    jobs.insert(name.clone(), job);
    save_saved_jobs(&jobs)?;

    let mut config = ApplicationConfig::load()?;
    config.job.default = Some(name);
    config.validate_references()?;
    config.save()?;
    Ok(config)
}

fn valid_output_host(host: &KnownHost) -> bool {
    host.app() == Some(Application::Elasticsearch)
        && host.has_role(HostRole::Send)
        && host
            .viewer()
            .and_then(|name| KnownHost::get_known(&name.to_string()))
            .is_some_and(|viewer| viewer.app() == Some(Application::Kibana) && viewer.has_role(HostRole::View))
}

fn validate_name(name: &str, kind: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(eyre!("{kind} name cannot be empty"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CollectHostInput, OnboardingReadiness, OutputDeploymentInput, inspect, save_collect_host,
        save_default_processing_job, save_output_deployment, save_user,
    };
    use crate::data::{Application, ApplicationConfig, SecretAuth, create_keystore};
    use url::Url;

    #[test]
    fn onboarding_operations_persist_a_linked_output_and_collect_host() {
        let _env = crate::TestEnv::new();
        create_keystore("pw").expect("create keystore");
        save_output_deployment(
            OutputDeploymentInput {
                output_name: "output-es".to_string(),
                output_url: Url::parse("https://es.example:9200").expect("url"),
                viewer_name: "output-kibana".to_string(),
                viewer_url: Url::parse("https://kb.example:5601").expect("url"),
                secret_id: "output-auth".to_string(),
                auth: SecretAuth::apikey("secret"),
            },
            "pw",
        )
        .expect("save output");
        save_collect_host(
            CollectHostInput {
                name: "collect-es".to_string(),
                app: Application::Elasticsearch,
                url: Url::parse("https://collect.example:9200").expect("url"),
                secret_id: None,
                auth: None,
            },
            None,
        )
        .expect("save collect host");

        let config = ApplicationConfig::load().expect("load config");
        let readiness = inspect().expect("inspect readiness");

        assert_eq!(config.output.default.as_deref(), Some("output-es"));
        assert!(readiness.keystore_ready);
        assert!(readiness.output_configured);
        assert!(readiness.collect_host_configured);
        assert!(!OnboardingReadiness::is_complete(&readiness));
    }

    #[test]
    fn readiness_resumes_after_independently_persisted_stages() {
        let _env = crate::TestEnv::new();
        create_keystore("pw").expect("create keystore");
        save_output_deployment(
            OutputDeploymentInput {
                output_name: "output-es".to_string(),
                output_url: Url::parse("https://es.example:9200").expect("url"),
                viewer_name: "output-kibana".to_string(),
                viewer_url: Url::parse("https://kb.example:5601").expect("url"),
                secret_id: "output-auth".to_string(),
                auth: SecretAuth::apikey("secret"),
            },
            "pw",
        )
        .expect("save output");
        save_collect_host(
            CollectHostInput {
                name: "collect-es".to_string(),
                app: Application::Elasticsearch,
                url: Url::parse("https://collect.example:9200").expect("url"),
                secret_id: None,
                auth: None,
            },
            None,
        )
        .expect("save collect host");

        assert!(!inspect().expect("partial readiness").is_complete());

        save_user("operator@example.com".to_string()).expect("save user");
        save_default_processing_job("default".to_string(), "collect-es".to_string()).expect("save default job");

        assert!(inspect().expect("completed readiness").is_complete());
    }
}
