use api::{app, config::Config, AppState};
use axum::http::{HeaderValue, Method};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    // Respects RUST_LOG if set; otherwise a sensible default. (EnvFilter::new, unlike
    // add_directive, parses a full comma-separated directive string in one call.)
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("api=debug,tower_http=debug"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let config = Config::from_env();
    let db = sqlx::PgPool::connect(&config.database_url)
        .await
        .expect("failed to connect to Postgres");

    let frontend_origin: HeaderValue = config
        .frontend_url
        .parse()
        .expect("FRONTEND_URL must be a valid origin");

    let state = AppState {
        db,
        http: reqwest::Client::new(),
        config,
    };

    // Cookies require `allow_credentials(true)`, which in turn forbids using `Any` for origin,
    // methods, or headers (tower_http panics at startup if it sees that combination) — so
    // everything below is an explicit list/mirror instead.
    let cors = CorsLayer::new()
        .allow_origin(frontend_origin)
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers(tower_http::cors::AllowHeaders::mirror_request());

    let router = app(state).layer(cors).layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
