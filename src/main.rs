use axum::{
    body::{self, Body},
    extract::{OriginalUri, Query, State},
    http::{HeaderMap, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::any,
    Router,
};
use axum::extract::connect_info::ConnectInfo;
use chrono::Utc;
use clap::Parser;
use serde_json::{json, Value};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Instant};
use tokio::{net::TcpListener, signal};
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
    about = env!("CARGO_PKG_DESCRIPTION")
)]
struct Args {
    /// Listening IP address
    #[arg(long, env = "BIND", default_value = "0.0.0.0")]
    bind: String,

    /// Port to listen on
    #[arg(short, long, env = "PORT", default_value = "3000")]
    port: u16,

    /// Combined bind address and port (overrides --bind and --port)
    #[arg(long, env = "LISTEN_ADDR")]
    addr: Option<SocketAddr>,

    /// Maximum request body size in bytes
    #[arg(long, env = "MAX_BODY_SIZE", default_value = "65536")] // 64KB
    max_body_size: usize,

    /// Server identification tag
    #[arg(long, env = "SERVER_TAG", default_value = "echo-server")]
    tag: String,
}

#[derive(Debug)]
struct AppConfig {
    max_body_size: usize,
    tag: String,
}

#[derive(Debug)]
enum EchoError {
    BodyTooLarge,
    BodyReadError(String),
    InvalidJson(String),
}

impl IntoResponse for EchoError {
    fn into_response(self) -> Response {
        let (status, message, details) = match self {
            EchoError::BodyTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE, 
                "Request body too large", 
                None
            ),
            EchoError::BodyReadError(err) => (
                StatusCode::BAD_REQUEST, 
                "Failed to read request body", 
                Some(err)
            ),
            EchoError::InvalidJson(err) => (
                StatusCode::BAD_REQUEST, 
                "Invalid JSON in request body", 
                Some(err)
            ),
        };

        let mut response_json = json!({
            "error": message,
            "timestamp": Utc::now().to_rfc3339()
        });

        if let Some(detail) = details {
            response_json["details"] = json!(detail);
        }

        let body = Json(response_json);
        (status, body).into_response()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_level(false)
        .init();

    let addr: SocketAddr = if let Some(addr) = args.addr {
        addr
    } else {
        format!("{}:{}", args.bind, args.port)
            .parse()
            .map_err(|e| format!("Invalid BIND or PORT: {}", e))?
    };

    // Validate body size
    if args.max_body_size > 10 * 1024 * 1024 {
        warn!("Max body size is quite large ({}MB), consider reducing it", args.max_body_size / (1024 * 1024));
    }

    let config = Arc::new(AppConfig {
        max_body_size: args.max_body_size,
        tag: args.tag,
    });

    let app = Router::new()
        .route("/", any(echo_handler))
        .route("/{*path}", any(echo_handler))
        .layer(middleware::from_fn(log_request))
        .with_state(config)
        .into_make_service_with_connect_info::<SocketAddr>();

    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to address {}: {}", addr, e))?;

    println!("✅ Rust test API server running at http://{}", addr);
    println!("📡 Press Ctrl+C to gracefully shutdown");

    // Graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("Server failed: {}", e))?;

    println!("🛑 Server shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            println!("\n🔄 Received Ctrl+C, initiating graceful shutdown...");
        },
        _ = terminate => {
            println!("\n🔄 Received SIGTERM, initiating graceful shutdown...");
        },
    }
}

async fn log_request(req: Request<Body>, next: Next) -> impl IntoResponse {
    let start = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let length = req
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("0")
        .to_string();
    let ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "-".to_string());

    let response = next.run(req).await;
    let status = response.status().as_u16();
    let duration = start.elapsed().as_millis();
    let timestamp = Utc::now().to_rfc3339();

    info!("{} {} \"{}  {}\" {} {} {}ms", timestamp, ip, method, path, status, length, duration);

    response
}

fn process_headers(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| {
            let key = name.to_string();
            let val = match value.to_str() {
                Ok(s) => s.to_string(),
                Err(_) => {
                    // Handle non-UTF8 headers by converting to lossy UTF-8
                    warn!("Non-UTF8 header value for key: {}", key);
                    String::from_utf8_lossy(value.as_bytes()).to_string()
                }
            };
            (key, val)
        })
        .collect()
}

async fn read_body(
    request: Request<Body>,
    max_size: usize,
) -> Result<Vec<u8>, EchoError> {
    match body::to_bytes(request.into_body(), max_size).await {
        Ok(bytes) => Ok(bytes.to_vec()),
        Err(e) => {
            error!("Failed to read request body: {}", e);
            if e.to_string().contains("too large") {
                Err(EchoError::BodyTooLarge)
            } else {
                Err(EchoError::BodyReadError(e.to_string()))
            }
        }
    }
}

fn parse_body(body_bytes: &[u8]) -> Result<Value, EchoError> {
    if body_bytes.is_empty() {
        return Ok(Value::Null);
    }

    let body_string = String::from_utf8_lossy(body_bytes);
    
    // Try to parse as JSON first
    match serde_json::from_str::<Value>(&body_string) {
        Ok(json_val) => Ok(json_val),
        Err(json_err) => {
            // If it's not valid JSON, check if it looks like it was intended to be JSON
            let trimmed = body_string.trim();
            if (trimmed.starts_with('{') && trimmed.ends_with('}')) ||
               (trimmed.starts_with('[') && trimmed.ends_with(']')) {
                warn!("Body looks like JSON but failed to parse: {}", json_err);
                Err(EchoError::InvalidJson(json_err.to_string()))
            } else {
                // Return as plain string if it doesn't look like JSON
                Ok(Value::String(body_string.to_string()))
            }
        }
    }
}

async fn echo_handler(
    State(config): State<Arc<AppConfig>>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Json<Value>, EchoError> {
    let headers_map = process_headers(&headers);
    
    let body_bytes = read_body(request, config.max_body_size).await?;
    let body_value = parse_body(&body_bytes)?;

    Ok(Json(json!({
        "method": method.to_string(),
        "path": uri.path().to_string(),
        "headers": headers_map,
        "query": query,
        "body": body_value,
        "server_tag": config.tag,
        "server_version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now().to_rfc3339(),
        "timestamp_unix": Utc::now().timestamp(),
    })))
}
