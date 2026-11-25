//! The HTTP side of the indexer: `/health`, `/manage/info` and `/metrics`. It asks the store
//! and the metrics and nothing else; the explorer API itself is Hasura over the database.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderName, StatusCode, header};
use axum::routing::get;
use serde_json::{Value, json};
use tracing::warn;

use crate::metrics::Metrics;
use crate::store::Store;

#[derive(Clone)]
struct ApiState {
    store: Store,
    metrics: Arc<Metrics>,
}

pub fn router(store: Store, metrics: Arc<Metrics>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/manage/info", get(manage_info))
        .route("/metrics", get(prometheus))
        .with_state(ApiState { store, metrics })
}

/// 200 only when the database answers. A node that is down does not fail this check: the
/// indexer keeps serving what it has and shows the lag on /metrics.
/// Docker calls it every 10 seconds, and it is one `SELECT 1`.
async fn health(State(state): State<ApiState>) -> StatusCode {
    match state.store.health().await {
        Ok(()) => StatusCode::OK,
        Err(e) => {
            warn!("health check failed: {e}");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

/// What runs here. The image has `GIT_COMMIT`, `GIT_BRANCH` and `BUILD_TIME` from its build
/// args (pouw-deploy `make images`, docker.yml); a binary built outside an image answers
/// `unknown` for them.
async fn manage_info() -> Json<Value> {
    Json(build_info(|name| std::env::var(name).ok()))
}

fn build_info(var: impl Fn(&str) -> Option<String>) -> Value {
    let field = |name: &str| {
        var(name)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "unknown".to_owned())
    };
    json!({
        "service": "pouw-indexer",
        "version": env!("CARGO_PKG_VERSION"),
        "commit": field("GIT_COMMIT"),
        "branch": field("GIT_BRANCH"),
        "build_time": field("BUILD_TIME"),
    })
}

async fn prometheus(State(state): State<ApiState>) -> ([(HeaderName, &'static str); 1], String) {
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        state.metrics.render(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_info_names_the_commit_branch_and_build_time_of_the_image() {
        let info = build_info(|name| match name {
            "GIT_COMMIT" => Some("0123456789abcdef".to_owned()),
            "GIT_BRANCH" => Some("feature/x".to_owned()),
            "BUILD_TIME" => Some(String::new()),
            _ => None,
        });
        assert_eq!(info["service"], "pouw-indexer");
        assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(info["commit"], "0123456789abcdef");
        assert_eq!(info["branch"], "feature/x");
        // set but empty (a build without the build arg) is unknown as well
        assert_eq!(info["build_time"], "unknown");
    }
}
