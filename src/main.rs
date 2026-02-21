mod config;
mod db;
mod error;
mod handlers;
mod middleware;
mod services;
mod stellar;

use axum::{
    Router,
    extract::State,
    middleware as axum_middleware,
    routing::{get, post},
};
use handlers::ws::TransactionStatusUpdate;
use middleware::idempotency::IdempotencyService;
use sqlx::migrate::Migrator; // for Migrator
use std::net::SocketAddr; // for SocketAddr
use std::path::Path; // for Path
use stellar::HorizonClient;
use tokio::net::TcpListener; // for TcpListener
use tokio::sync::broadcast;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt}; // for .with() on registry

#[derive(Clone)] // <-- Add Clone
pub struct AppState {
    db: sqlx::PgPool,
    pub horizon_client: HorizonClient,
    pub tx_broadcast: broadcast::Sender<TransactionStatusUpdate>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::from_env()?;

    // Setup logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Database pool
    let pool = db::create_pool(&config).await?;

    // Run migrations
    let migrator = Migrator::new(Path::new("./migrations")).await?;
    migrator.run(&pool).await?;
    tracing::info!("Database migrations completed");

    // Initialize Stellar Horizon client
    let horizon_client = HorizonClient::new(config.stellar_horizon_url.clone());
    tracing::info!(
        "Stellar Horizon client initialized with URL: {}",
        config.stellar_horizon_url
    );

    // Initialize Redis idempotency service
    let idempotency_service = IdempotencyService::new(&config.redis_url)?;
    tracing::info!("Redis idempotency service initialized");

    // Create broadcast channel for WebSocket notifications
    // Channel capacity of 100 - slow clients will miss old messages (backpressure handling)
    let (tx_broadcast, _) = broadcast::channel::<TransactionStatusUpdate>(100);
    tracing::info!("WebSocket broadcast channel initialized");

    // Build router with state
    let app_state = AppState {
        db: pool,
        horizon_client,
        tx_broadcast,
    };

    // Create webhook routes with idempotency middleware
    let webhook_routes = Router::new()
        .route("/webhook", post(handlers::webhook::handle_webhook))
        .layer(axum_middleware::from_fn_with_state(
            idempotency_service.clone(),
            middleware::idempotency::idempotency_middleware,
        ))
        .with_state(app_state.clone());

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/ws/transactions", get(handlers::ws::ws_handler))
        .merge(webhook_routes)
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    tracing::info!("listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
