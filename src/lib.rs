pub mod config;
pub mod db;
pub mod error;
pub mod handlers;
pub mod services;
pub mod stellar;
pub mod graphql;
pub mod schemas;
pub mod middleware;

use axum::{Router, routing::{get, post}};
use crate::stellar::HorizonClient;
use crate::graphql::schema::{AppSchema, build_schema};

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub horizon_client: HorizonClient,
}

#[derive(Clone)]
pub struct ApiState {
    pub app_state: AppState,
    pub graphql_schema: AppSchema,
}

pub fn create_app(app_state: AppState) -> Router {
    let graphql_schema = build_schema(app_state.clone());
    let state = ApiState {
        app_state: app_state.clone(),
        graphql_schema: graphql_schema.clone(),
    };

    // V1 routes
    let v1_router = Router::new()
        .route("/health", get(handlers::v1::health))
        .route("/webhook", post(handlers::v1::webhook::handle_webhook))
        .route("/callback/transaction", post(handlers::v1::webhook::callback))
        .route("/transactions/:id", get(handlers::v1::webhook::get_transaction))
        .layer(axum::middleware::from_fn(middleware::versioning::inject_deprecation_headers))
        .with_state(state.clone());

    // V2 routes
    let v2_router = Router::new()
        .route("/health", get(handlers::v2::health))
        .route("/webhook", post(handlers::v2::webhook::handle_webhook))
        .route("/transactions/:id", get(handlers::v2::webhook::get_transaction))
        .with_state(state.clone());

    Router::new()
        .route("/health", get(handlers::health))
        .route("/settlements", get(handlers::settlements::list_settlements))
        .route("/settlements/:id", get(handlers::settlements::get_settlement))
        .route("/callback", post(handlers::webhook::callback))
        .route("/callback/transaction", post(handlers::webhook::callback)) // Backward compatibility
        .route("/transactions/:id", get(handlers::webhook::get_transaction))
        .route("/graphql", post(handlers::graphql::graphql_handler).get(handlers::graphql::subscription_handler))
        .route("/graphql/playground", get(handlers::graphql::graphql_playground))
        .nest("/v1", v1_router)
        .nest("/v2", v2_router)
        .with_state(state)
}
