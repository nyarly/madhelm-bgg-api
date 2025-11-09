use std::{net::SocketAddr, num::ParseIntError, time::Duration};

use axum::{extract, response::IntoResponse, routing::get, Router};
use clap::Parser;
use mattak::{
    biscuits::{self, keysets::{AuthorityMap, KeyMap}}, cachecontrol::CacheControlLayer, ratelimiting::{self, GovernorConfigBuilder, IpExtractor}
};
use biscuit_auth::macros::authorizer;
use reqwest::{header, Certificate, Client, Method, StatusCode};
use resources::{api_doc, branding, search, thing};
use sqlx::{postgres::{PgConnectOptions, PgPoolOptions}, Pool, Postgres};
use tracing::debug;
use tracing_subscriber::{EnvFilter, prelude::*};
use tower_http::{trace::TraceLayer, cors::CorsLayer};


mod resources;
mod db;
mod bgg_api;

#[derive(Parser)]
struct Config {
    #[arg(long, env = "LOCAL_ADDR", default_value = "127.0.0.1:3001")]
    local_addr: String,

    /// Canonical domain the site is served from. Will be used in messages sent via email
    #[arg(long, env = "CANON_DOMAIN")]
    canon_domain: String,

    #[arg(long, env = "DATABASE_URL")]
    db_connection_str: String,

    #[arg(long, env = "TRUST_FORWARDED_HEADER", default_value = "false")]
    trust_forwarded_header: bool,

    #[arg(long, env = "BGG_API_TOKEN")]
    bgg_api_token: String,

    #[arg(long, env = "BGG_SIMULTANEUS_REQUESTS", default_value = "10")]
    bgg_simultaneus_requests: usize,

    #[arg(long, env = "AUTH_MAP")]
    auth_map: String,

    #[arg(long, env = "CORS_ORIGINS")]
    cors_origins: String

}

#[derive(Clone)]
struct BggLimit(usize);

impl From<BggLimit> for usize {
    fn from(value: BggLimit) -> Self {
        value.0
    }
}

#[derive(extract::FromRef, Clone)]
struct AppState {
    pool: Pool<Postgres>,
    client: Client,
    bgg_limit: BggLimit,
    key_map: KeyMap,
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let config = Config::parse();

    debug!("{:?}", config.db_connection_str);
    let dbopts: PgConnectOptions = config.db_connection_str.parse().expect("couldn't parse DATABASE_URL");

    debug!("{:?}", dbopts);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with(dbopts)
        .await
        .expect("can't connect to database");

    let mut headers = header::HeaderMap::new();
    let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {}", config.bgg_api_token))?;
    auth_value.set_sensitive(true);
    headers.insert(header::AUTHORIZATION, auth_value);

    let client = Client::builder()
        .use_rustls_tls()
        .default_headers(headers)
        .build()?;

    let bgg_limit = BggLimit(config.bgg_simultaneus_requests);

    let path = "../devsupport/tls/wtp/ca.crt.pem";
    let data = std::fs::read(path)?;
    let wtp_cert = Certificate::from_pem(&data)?;
    let key_client = Client::builder()
        .use_rustls_tls()
        .add_root_certificate(wtp_cert)
        .build()?;
    let key_map = AuthorityMap::from(parse_auth_map(&config.auth_map)).fetch_keys(key_client).await?;

    debug!("{key_map:?}");
    let state = AppState{pool, client, bgg_limit, key_map: key_map.clone()};

    let rate_key = IpExtractor::trust(config.trust_forwarded_header);

    let app = Router::new()
        .nest("/api", root_api_router(rate_key, key_map, parse_cors_origins(&config.cors_origins)));

    let app = app
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.local_addr.to_string()).await.expect("couldn't bind on local addr");
    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?; Ok(())
}

fn parse_auth_map(cfg: &str) -> Vec<(&str, &str)> {
    cfg.split(",").map(|mapping| {
        let mut pair = mapping.splitn(2, "=");
        let left = pair.next().expect("must have key");
        let right = pair.next().expect("must have value");
        (left,right)
    }).collect()
}

fn parse_cors_origins(cfg: &str) -> Vec<header::HeaderValue> {
    cfg.split(",").map(|origin| {
        origin.parse().expect("parse origin")
    }).collect()
}

fn root_api_router(extractor: IpExtractor, auth: KeyMap, origin_list: Vec<header::HeaderValue>) -> Router<AppState> {
    let cors = CorsLayer::new()
        // .max_age(Duration::from_secs(60))
        .allow_credentials(true)
        .allow_headers([header::AUTHORIZATION, header::ACCEPT])
        .allow_methods([Method::GET])
        .allow_origin(origin_list);

    open_api_router()
        .merge(authenticated_router(auth))
        .layer(tower::ServiceBuilder::new()
            .layer(cors)
            .layer(ratelimiting::layer("api-root", extractor, GovernorConfigBuilder::default()
                .per_millisecond(20)
                .burst_size(60)
            ))
            .layer(CacheControlLayer::new(30))
        )
        // XXX key extractor that is either Authentication or SmartIp
}

fn open_api_router() -> Router<AppState> {
    Router::new()
        .route(&api_doc::route(), get(api_doc::get))
        .route(&branding::list_route(), get(branding::get))
        .nest_service(&branding::route(), branding::logos())
}

fn authenticated_router(auth: KeyMap) -> Router<AppState> {
    Router::new()
        .route(&search::route(), get(search::get))
        .route(&thing::route(), get(thing::get))
        .layer(tower::ServiceBuilder::new()
            .layer(biscuits::middleware::setup(auth, "Authorization"))
            // .layer(middleware::from_fn_with_state(state, authentication::add_rejections))
            .layer(biscuits::middleware::check(authorizer!(r#"allow if user($user);"#)))
        )
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    // #[error("database error: ${0:?}")]
    // DB(#[from] db::Error),
    #[error("http error: ${0:?}")]
    HTTP(#[from] mattak::Error),
    #[error("status code: ${0:?} - ${1}")]
    StatusCode(StatusCode, String),
    // #[error("cryptographic issue: ${0:?}")]
    // Crypto(#[from] bcrypt::BcryptError),
    #[error("Problem with job queue: ${0:?}")]
    Job(String),
    // #[error("Problem setting up email: ${0:?}")]
    // Email(#[from] mailing::Error),
    #[error("Couldn't serialize data: ${0:?}")]
    Serialization(#[from] serde_json::Error),
    #[error("Problem with upstream API: ${0:?}")]
    Client(#[from] reqwest::Error),
    #[error("XML parse error: {0}")]
    XML(#[from] quick_xml::Error),
    #[error("Converting API string result into a string: {0}")]
    ParseInt(#[from] ParseIntError),
    #[error("Did not find expected data in BGG API response")] // go figure
    MalformedResponse,
    #[error("Too many retries, gave up: {0:?}")]
    GivingUp(StatusCode),
    #[error("API server said: {0:?}")]
    Upstream(StatusCode)
}


impl From<(StatusCode, &'static str)> for Error {
    fn from((code, text): (StatusCode, &'static str)) -> Self {
        Self::StatusCode(code, text.to_string())

    }
}

impl From<(StatusCode, String)> for Error {
    fn from((code, text): (StatusCode, String)) -> Self {
        Self::StatusCode(code, text)

    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::HTTP(e) => e.into_response(),
            Error::StatusCode(c, t) => (c,t).into_response(),
            Error::Job(m) => (StatusCode::INTERNAL_SERVER_ERROR, m).into_response(),
            Error::Serialization(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", e)).into_response(),
            Error::GivingUp(_) |
            Error::Upstream(_) |
            Error::MalformedResponse |
            Error::Client(_) |
            Error::XML(_) |
            Error::ParseInt(_) => (StatusCode::BAD_GATEWAY, format!("{self}")).into_response(),
        }
    }
}
