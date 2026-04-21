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

/// Spawn the background mail-monitoring task.
///
/// Strategy depends on the configured mail protocol:
///
/// * **IMAP** — uses RFC 2177 IDLE for push-based notification.  The server
///   pushes an unsolicited `EXISTS`/`RECENT` response the instant new mail
///   arrives; we immediately re-fetch and broadcast an SSE `refresh` event.
///   IDLE is re-issued every 28 minutes as required by the RFC.  If the IDLE
///   connection drops, we wait 5 s and reconnect.
///
/// * **POP3 / unconfigured** — falls back to timed polling on the interval
///   configured via `POP3_POLL_INTERVAL` (default 30 s).
fn spawn_refresh_task(state: AppState) {
    if state.config.imap.is_some() {
        spawn_imap_idle_task(state);
    } else {
        spawn_polling_task(state);
    }
}

/// IMAP IDLE background task.
fn spawn_imap_idle_task(state: AppState) {
    tokio::spawn(async move {
        // Reconnect delay on error (seconds).
        const RECONNECT_DELAY_SECS: u64 = 5;

        loop {
            // Wait until a user is logged in before starting IDLE.
            let session = loop {
                let s = {
                    let inner = state.inner.read().await;
                    inner.session.clone()
                };
                match s {
                    Some(s) => break s,
                    None => tokio::time::sleep(std::time::Duration::from_secs(2)).await,
                }
            };

            let imap_cfg = match &state.config.imap {
                Some(c) => c.clone(),
                None => return, // should not happen
            };

            // Do an initial fetch so conversations are populated on login.
            refresh_conversations(&state, &session.email, &session.password).await;

            // Clone what the closure needs.
            let state_for_cb = state.clone();
            let email_for_cb = session.email.clone();
            let password_for_cb = session.password.clone();

            tracing::info!("Starting IMAP IDLE for {}", session.email);

            let result = crate::email::imap::idle_loop(
                &imap_cfg,
                &session.email,
                &session.password,
                move || {
                    // This closure is called from within an async context (the
                    // IDLE future), so we spawn a new task to do the async work.
                    let s = state_for_cb.clone();
                    let e = email_for_cb.clone();
                    let p = password_for_cb.clone();
                    tokio::spawn(async move {
                        refresh_conversations(&s, &e, &p).await;
                    });
                },
            )
            .await;

            match result {
                Ok(()) => {
                    tracing::debug!("IMAP IDLE loop exited cleanly");
                }
                Err(e) => {
                    tracing::warn!(
                        "IMAP IDLE error: {e}; reconnecting in {RECONNECT_DELAY_SECS}s"
                    );
                }
            }

            // Check whether the user is still logged in before reconnecting.
            let still_logged_in = state.inner.read().await.session.is_some();
            if !still_logged_in {
                continue; // will wait for login again at top of outer loop
            }

            tokio::time::sleep(std::time::Duration::from_secs(RECONNECT_DELAY_SECS)).await;
        }
    });
}

/// Polling-based background task (POP3 / unconfigured).
fn spawn_polling_task(state: AppState) {
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
                refresh_conversations(&state, &session.email, &session.password).await;
            }
        }
    });
}

/// Fetch conversations and broadcast an SSE refresh event.
async fn refresh_conversations(state: &AppState, email: &str, password: &str) {
    match crate::email::mail_service::fetch_conversations(&state.config, email, password).await {
        Ok(convos) => {
            let mut inner = state.inner.write().await;
            inner.conversations = convos;
            state
                .events
                .send(crate::state::AppEvent::ConversationsUpdated)
                .ok();
        }
        Err(e) => {
            tracing::warn!("Conversation refresh failed: {e}");
        }
    }
}

