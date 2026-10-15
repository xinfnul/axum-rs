use std::{net::SocketAddr, time::Duration};

use anyhow::Context;
use auth::config::Config;
use axum::{Json, Router, http::StatusCode, routing::get};
use tokio::signal;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "{}=debug,tower_http=debug,axum=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer().without_time())
        .init();

    let _config = Config::from_env()?;

    // tracing::info!(config = ?config, "server configuration");

    let app: Router<_> = Router::new()
        .route(
            "/",
            get(|| async { Json(serde_json::json!({"status": "OK"})) }),
        )
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "resource not found",})),
            )
        })
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .layer(TraceLayer::new_for_http());

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "5000".to_string())
        .parse()
        .context("PORT must be a valid port number")?;

    let addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], port));

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind {}", addr))?;

    tracing::info!(addr = %addr, "listening on");

    let _ = axum::serve(listener, app)
        .with_graceful_shutdown(async {
            if let Err(error) = shutdown_signal().await {
                tracing::error!(%error, "shutdown handler failed");
            }
        })
        .await
        .context("HTTP server failed")?;

    Ok(())
}

async fn shutdown_signal() -> anyhow::Result<()> {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .context("failed to install Ctrl+C handler")?;

        Ok::<(), anyhow::Error>(())
    };

    #[cfg(unix)]
    let terminate = async {
        let mut terminate = signal::unix::signal(signal::unix::SignalKind::terminate())
            .context("failed to install SIGTERM handler")?;

        terminate.recv().await;

        Ok::<(), anyhow::Error>(())
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<anyhow::Result<()>>();

    tokio::select! {
        result = ctrl_c => result?,
        result = terminate => result?,
    }

    tracing::info!("shutdown signal received");

    Ok(())
}
