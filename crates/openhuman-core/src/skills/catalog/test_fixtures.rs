//! A loopback Hermes index and `SKILL.md` host for registry tests.

use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde_json::{json, Value};
use tinyskills::{Clock, FetchPolicy, HermesIndexSource, SkillRegistry};

use super::registry::RegistryConfig;
use super::transport::ReqwestTransport;

#[derive(Clone)]
pub(crate) struct Fixture {
    pub(crate) base: String,
    pub(crate) catalog_hits: Arc<AtomicUsize>,
    pub(crate) catalog_status: Arc<AtomicU16>,
    pub(crate) document_status: Arc<AtomicU16>,
}

#[derive(Clone)]
struct FixtureState {
    catalog: Arc<Mutex<Value>>,
    catalog_hits: Arc<AtomicUsize>,
    catalog_status: Arc<AtomicU16>,
    document_status: Arc<AtomicU16>,
}

async fn catalog_route(State(state): State<FixtureState>) -> Response {
    state.catalog_hits.fetch_add(1, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(30)).await;
    let status = state.catalog_status.load(Ordering::SeqCst);
    if status != 200 {
        return StatusCode::from_u16(status).unwrap().into_response();
    }
    axum::Json(state.catalog.lock().unwrap().clone()).into_response()
}

async fn document_route(
    State(state): State<FixtureState>,
    AxumPath(name): AxumPath<String>,
) -> Response {
    let status = state.document_status.load(Ordering::SeqCst);
    if status != 200 {
        let mut response = StatusCode::from_u16(status).unwrap().into_response();
        if status == 429 {
            response
                .headers_mut()
                .insert("retry-after", "42".parse().unwrap());
        }
        return response;
    }
    format!("---\nname: {name}\ndescription: Fixture skill {name}.\n---\n\n# {name}\n")
        .into_response()
}

pub(crate) fn hermes_item(name: &str, source: &str) -> Value {
    json!({
        "name": name,
        "description": format!("{name} helper"),
        "category": "productivity",
        "source": source,
        "tags": ["fixture"],
        "docsPath": format!("fixture/productivity/{name}"),
    })
}

impl Fixture {
    pub(crate) async fn start(items: Vec<Value>) -> Self {
        let state = FixtureState {
            catalog: Arc::new(Mutex::new(Value::Array(items))),
            catalog_hits: Arc::new(AtomicUsize::new(0)),
            catalog_status: Arc::new(AtomicU16::new(200)),
            document_status: Arc::new(AtomicU16::new(200)),
        };
        let app = Router::new()
            .route("/skills.json", get(catalog_route))
            .route("/skills/{name}/SKILL.md", get(document_route))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base: format!("http://{addr}"),
            catalog_hits: state.catalog_hits,
            catalog_status: state.catalog_status,
            document_status: state.document_status,
        }
    }

    pub(crate) fn registry(&self) -> Arc<SkillRegistry> {
        self.build(tinyskills::SystemClock, true)
    }

    pub(crate) fn registry_without_download_base(&self) -> Arc<SkillRegistry> {
        self.build(tinyskills::SystemClock, false)
    }

    pub(crate) fn registry_with_clock(&self, clock: impl Clock + 'static) -> Arc<SkillRegistry> {
        self.build(clock, true)
    }

    fn build(&self, clock: impl Clock + 'static, download_base: bool) -> Arc<SkillRegistry> {
        let mut policy = FetchPolicy::default();
        policy.allow_loopback_http = true;
        let source = HermesIndexSource::new("hermes", format!("{}/skills.json", self.base));
        let source = if download_base {
            source.with_download_base(format!("{}/skills", self.base))
        } else {
            source
        };
        SkillRegistry::builder(ReqwestTransport::new())
            .source(source)
            .policy(policy)
            .limits(RegistryConfig::limits())
            .clock(clock)
            .build()
    }
}
