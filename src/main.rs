/// RoundChat — chat built on the universal language of the internet: email.
///
/// Starts a local HTTP server that serves the HTML/CSS/JS frontend and
/// exposes REST + SSE API endpoints.  Automatically opens the default browser
/// on startup.  All server configuration is read from environment variables
/// (see .env.example).

mod api;
mod config;
mod dav;
mod email;
mod models;
mod state;

use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialise logging (RUST_LOG env var, default to info).
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "roundchat=info,tower_http=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration from environment variables.
    let config = AppConfig::from_env();
    let port = config.port;

    let state = AppState::new(config);

    // Start background refresh task.
    spawn_refresh_task(state.clone());

    // Build router.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = api::router(state).layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;
    let url = format!("http://{actual_addr}");

    tracing::info!("RoundChat listening on {url}");

    // Open the browser (best-effort; ignore errors on headless systems).
    let url_clone = url.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = open::that(&url_clone) {
            tracing::warn!("Could not open browser: {e}");
        }
    });

    axum::serve(listener, app).await?;
    Ok(())
}

/// Spawn a background task that periodically refreshes conversations and
/// broadcasts an SSE event to connected clients.
fn spawn_refresh_task(state: AppState) {
    // Use POP3 poll interval if configured; otherwise default to 30 s.
    let interval_secs = state
        .config
        .pop3
        .as_ref()
        .map(|p| p.poll_interval_secs)
        .unwrap_or(30);

    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;

            let session = {
                let inner = state.inner.read().await;
                inner.session.clone()
            };

            if let Some(session) = session {
                match crate::email::mail_service::fetch_conversations(
                    &state.config,
                    &session.email,
                    &session.password,
                )
                .await
                {
                    Ok(convos) => {
                        let mut inner = state.inner.write().await;
                        inner.conversations = convos;
                        state
                            .events
                            .send(crate::state::AppEvent::ConversationsUpdated)
                            .ok();
                    }
                    Err(e) => {
                        tracing::warn!("Background refresh failed: {e}");
                    }
                }
            }
        }
    });
}
