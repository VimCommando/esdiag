// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

pub const ESDIAG_ES_BULK_SIZE: usize = 10_000;
pub const ESDIAG_ES_BULK_BYTES: usize = 50 * 1024 * 1024;
pub const ESDIAG_ES_WORKERS: usize = 4;
pub static ESDIAG_HOME: &str = ".esdiag";
pub static LOG_LEVEL: &str = "info";
pub static ESDIAG_KIBANA_URL: &str = "http://localhost:5601";
pub static ESDIAG_KIBANA_DEFAULT_SPACE: &str = "esdiag";
pub static ESDIAG_KEYSTORE_PASSWORD: &str = "ESDIAG_KEYSTORE_PASSWORD";

fn default_int(name: &str) -> Option<usize> {
    match name {
        "ESDIAG_ES_BULK_BYTES" => Some(ESDIAG_ES_BULK_BYTES),
        "ESDIAG_ES_BULK_SIZE" => Some(ESDIAG_ES_BULK_SIZE),
        "ESDIAG_ES_WORKERS" => Some(ESDIAG_ES_WORKERS),
        _ => None,
    }
}

fn default_str(name: &str) -> Option<&str> {
    match name {
        "ESDIAG_HOME" => Some(ESDIAG_HOME),
        "LOG_LEVEL" => Some(LOG_LEVEL),
        "ESDIAG_KIBANA_URL" => Some(ESDIAG_KIBANA_URL),
        _ => None,
    }
}

pub fn get_int(name: &str) -> std::io::Result<usize> {
    let env = std::env::var(name).ok().and_then(|s| s.parse::<usize>().ok());
    let default = default_int(name);

    env.or(default)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, format!("{} not found", name)))
}

pub fn get_string(name: &str) -> std::io::Result<String> {
    let env = std::env::var(name).ok();
    let default = default_str(name);

    env.or(default.map(|s| s.to_string()))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, format!("{} not found", name)))
}

pub fn get_kibana_space() -> Option<String> {
    match std::env::var("ESDIAG_KIBANA_SPACE") {
        Ok(space) => {
            let trimmed = space.trim();
            if trimmed.is_empty() || trimmed == "_default" || trimmed == "default" {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => Some(ESDIAG_KIBANA_DEFAULT_SPACE.to_string()),
    }
}

pub fn append_kibana_space(kibana_url: &str) -> String {
    kibana_url_with_space(kibana_url, get_kibana_space().as_deref())
}

pub fn kibana_url_with_space(kibana_url: &str, space: Option<&str>) -> String {
    let kibana_url = kibana_url.trim_end_matches('/');
    let space = space
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "_default" && *s != "default");
    match space {
        Some(space) => {
            if let Ok(mut url) = url::Url::parse(kibana_url)
                && let Some(existing_segments) = url.path_segments()
            {
                let mut segments = existing_segments
                    .filter(|segment| !segment.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                if let Some(index) = segments.windows(2).rposition(|pair| pair[0] == "s") {
                    segments[index + 1] = space.to_string();
                } else if segments.last().map(String::as_str) == Some("s") {
                    segments.push(space.to_string());
                } else {
                    let insert_at = segments
                        .iter()
                        .position(|segment| matches!(segment.as_str(), "app" | "api" | "internal"))
                        .unwrap_or(segments.len());
                    segments.insert(insert_at, space.to_string());
                    segments.insert(insert_at, "s".to_string());
                }
                url.set_path(&format!("/{}", segments.join("/")));
                return url.to_string();
            }
            format!("{kibana_url}/s/{space}")
        }
        None => {
            if let Ok(mut url) = url::Url::parse(kibana_url) {
                let mut segments = url
                    .path_segments()
                    .map(|segments| {
                        segments
                            .filter(|segment| !segment.is_empty())
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if let Some(index) = segments.windows(2).rposition(|pair| pair[0] == "s") {
                    segments.drain(index..=index + 1);
                    let path = format!("/{}", segments.join("/"));
                    url.set_path(&path);
                    return url.to_string();
                } else if segments.last().map(String::as_str) == Some("s") {
                    segments.pop();
                    let path = format!("/{}", segments.join("/"));
                    url.set_path(&path);
                    return url.to_string();
                }
            }
            kibana_url.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{append_kibana_space, get_kibana_space};
    use std::sync::Mutex;

    fn env_lock() -> &'static Mutex<()> {
        crate::test_env_lock()
    }

    #[test]
    fn explicit_default_space_removes_existing_prefix() {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var_os("ESDIAG_KIBANA_SPACE");
        for value in ["_default", "", "  _default  ", "default"] {
            unsafe {
                std::env::set_var("ESDIAG_KIBANA_SPACE", value);
            }
            assert_eq!(get_kibana_space(), None);
            assert_eq!(
                append_kibana_space("https://kb/s/ops/app/home?x=1#hash"),
                "https://kb/app/home?x=1#hash"
            );
            assert_eq!(append_kibana_space("https://kb/s/ops"), "https://kb/");
            assert_eq!(
                append_kibana_space("https://kb/proxy/s/ops/app/home"),
                "https://kb/proxy/app/home"
            );
        }
        unsafe {
            match previous {
                Some(value) => std::env::set_var("ESDIAG_KIBANA_SPACE", value),
                None => std::env::remove_var("ESDIAG_KIBANA_SPACE"),
            }
        }
    }

    #[test]
    fn default_kibana_space_is_esdiag() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::remove_var("ESDIAG_KIBANA_SPACE");
        }

        assert_eq!(get_kibana_space().as_deref(), Some("esdiag"));
    }

    #[test]
    fn append_kibana_space_replaces_existing_space_and_preserves_path() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("ESDIAG_KIBANA_SPACE", "support");
        }

        assert_eq!(
            append_kibana_space("https://kb:5601/s/foo/app/home"),
            "https://kb:5601/s/support/app/home"
        );
        assert_eq!(
            append_kibana_space("https://kb:5601/proxy/s/foo/app/home"),
            "https://kb:5601/proxy/s/support/app/home"
        );
        assert_eq!(
            append_kibana_space("https://kb:5601/proxy/app/home"),
            "https://kb:5601/proxy/s/support/app/home"
        );
        assert_eq!(
            append_kibana_space("https://kb:5601/proxy/s"),
            "https://kb:5601/proxy/s/support"
        );
    }

    #[test]
    fn append_kibana_space_inserts_space_before_existing_path() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("ESDIAG_KIBANA_SPACE", "support");
        }

        assert_eq!(
            append_kibana_space("https://kb:5601/app/home?foo=bar#hash"),
            "https://kb:5601/s/support/app/home?foo=bar#hash"
        );
    }

    #[test]
    fn append_kibana_space_omits_space_segment_when_env_is_empty() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("ESDIAG_KIBANA_SPACE", "");
        }

        assert_eq!(
            append_kibana_space("https://kb:5601/app/home"),
            "https://kb:5601/app/home"
        );
        assert_eq!(append_kibana_space("https://kb:5601/proxy/s"), "https://kb:5601/proxy");
    }
}
