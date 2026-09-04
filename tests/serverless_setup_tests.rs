// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

#![cfg(feature = "setup")]

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
};
use esdiag::client::{Client, ElasticsearchBuilder};
use std::sync::{Arc, Mutex};

struct MockCluster {
    client: Client,
    paths: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

#[tokio::test]
async fn deployment_metadata_handles_both_products_and_restricted_metadata() {
    for kibana in [false, true] {
        for (status, body, expected) in [
            (
                StatusCode::OK,
                r#"{"version":{"build_flavor":"serverless"}}"#,
                Some(true),
            ),
            (StatusCode::OK, r#"{"version":{"build_flavor":"default"}}"#, Some(false)),
            (StatusCode::OK, r#"{"version":{"number":"8.19.0"}}"#, Some(false)),
            (StatusCode::FORBIDDEN, "", Some(false)),
            (StatusCode::INTERNAL_SERVER_ERROR, "", None),
            (StatusCode::OK, "invalid", None),
        ] {
            let app = Router::new().fallback(move |request: Request<Body>| async move {
                assert_eq!(request.uri().path(), if kibana { "/api/status" } else { "/" });
                (
                    status,
                    [
                        ("content-type", "application/json"),
                        ("x-elastic-product", "Elasticsearch"),
                    ],
                    body,
                )
            });
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap()).parse().unwrap();
            let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let client = if kibana {
                Client::Kibana(esdiag::client::KibanaClient::try_new(url, esdiag::data::Auth::None).unwrap())
            } else {
                Client::Elasticsearch(ElasticsearchBuilder::new(url).build().unwrap())
            };
            let actual = client.is_serverless().await;
            task.abort();
            assert_eq!(actual.ok(), expected);
        }
    }
}

impl Drop for MockCluster {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn cluster(probe_status: StatusCode, probe_body: &'static str, asset_status: StatusCode) -> MockCluster {
    cluster_with_flavor(probe_status, probe_body, asset_status, probe_status == StatusCode::GONE).await
}

async fn cluster_with_flavor(
    probe_status: StatusCode,
    probe_body: &'static str,
    asset_status: StatusCode,
    serverless: bool,
) -> MockCluster {
    let paths = Arc::new(Mutex::new(Vec::new()));
    let requests = paths.clone();
    let app = Router::new().fallback(move |request: Request<Body>| {
        let requests = requests.clone();
        async move {
            assert_eq!(request.headers()["authorization"], "ApiKey test-key");
            let path = request.uri().path().to_string();
            requests.lock().unwrap().push(path.clone());
            let (status, body) = if path == "/" {
                (
                    StatusCode::OK,
                    if serverless {
                        r#"{"version":{"build_flavor":"serverless"}}"#
                    } else {
                        r#"{"version":{"build_flavor":"default"}}"#
                    },
                )
            } else if path == "/_xpack/usage" {
                (probe_status, probe_body)
            } else if path.ends_with("/_mapping") {
                (StatusCode::OK, "{}")
            } else if path.starts_with("/_security/") && probe_status == StatusCode::GONE {
                (StatusCode::GONE, r#"{"error":"unsupported role"}"#)
            } else {
                (asset_status, r#"{"acknowledged":true}"#)
            };
            (
                status,
                [
                    ("content-type", "application/json"),
                    ("x-elastic-product", "Elasticsearch"),
                ],
                body,
            )
                .into_response()
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap()).parse().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::Elasticsearch(
        ElasticsearchBuilder::new(url)
            .apikey("test-key".into())
            .build()
            .unwrap(),
    );
    MockCluster { client, paths, task }
}

#[tokio::test]
async fn security_probe_preserves_stateful_behavior_and_handles_gone() {
    for (status, body, expected) in [
        (StatusCode::OK, r#"{"security":{"enabled":true}}"#, true),
        (StatusCode::OK, r#"{"security":{"enabled":false}}"#, false),
        (StatusCode::OK, "{}", true),
        (StatusCode::UNAUTHORIZED, "", true),
        (StatusCode::FORBIDDEN, "", true),
        (StatusCode::NOT_FOUND, "", false),
        (StatusCode::GONE, "not JSON", true),
    ] {
        let mock = cluster(status, body, StatusCode::OK).await;
        assert_eq!(mock.client.has_security_enabled().await.unwrap(), expected);
    }
    for (status, body) in [
        (StatusCode::INTERNAL_SERVER_ERROR, ""),
        (StatusCode::OK, "invalid JSON"),
    ] {
        let mock = cluster(status, body, StatusCode::OK).await;
        assert!(mock.client.has_security_enabled().await.is_err());
    }
}

#[tokio::test]
async fn setup_installs_assets_and_skips_roles_only_when_needed() {
    for (status, body, expects_roles) in [
        (StatusCode::GONE, "", false),
        (StatusCode::OK, r#"{"security":{"enabled":false}}"#, false),
        (StatusCode::OK, r#"{"security":{"enabled":true}}"#, true),
        (StatusCode::FORBIDDEN, "", true),
    ] {
        let mock = cluster(status, body, StatusCode::OK).await;
        esdiag::setup::assets(&mock.client).await.unwrap();
        let paths = mock.paths.lock().unwrap();
        assert_eq!(
            paths.iter().filter(|p| *p == "/_xpack/usage").count(),
            usize::from(status != StatusCode::GONE)
        );
        for prefix in ["/_ingest/pipeline/", "/_component_template/", "/_index_template/"] {
            assert!(paths.iter().any(|p| p.starts_with(prefix)), "missing {prefix}");
        }
        assert_eq!(paths.iter().any(|p| p.starts_with("/_security/role/")), expects_roles);
    }
}

#[tokio::test]
async fn setup_still_reports_probe_and_asset_failures() {
    let mock = cluster(StatusCode::INTERNAL_SERVER_ERROR, "", StatusCode::OK).await;
    assert!(esdiag::setup::assets(&mock.client).await.is_err());
    assert_eq!(*mock.paths.lock().unwrap(), vec!["/", "/_xpack/usage"]);

    let mock = cluster(StatusCode::GONE, "", StatusCode::FORBIDDEN).await;
    assert!(esdiag::setup::assets(&mock.client).await.is_err());
}

#[tokio::test]
async fn serverless_metadata_avoids_stateful_security_and_license_apis() {
    let mock = cluster_with_flavor(StatusCode::INTERNAL_SERVER_ERROR, "", StatusCode::OK, true).await;
    esdiag::setup::assets(&mock.client).await.unwrap();
    esdiag::setup::ensure_agent_builder_license(&mock.client).await.unwrap();
    let paths = mock.paths.lock().unwrap();
    assert!(
        !paths
            .iter()
            .any(|p| p.starts_with("/_security/") || p == "/_xpack/usage" || p.starts_with("/_license"))
    );
    assert!(paths.iter().any(|p| p.starts_with("/_index_template/")));
}
