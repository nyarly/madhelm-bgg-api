use axum::{response::IntoResponse, Json};
use include_dir::{include_dir,Dir};
use mattak::routing::{extract::NestedRoute, Route as _};
use mattak_derives::Route;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tower_serve_static::ServeDir;

use crate::Error;

pub(crate) fn route() -> String {
    "/logos".to_string()
}

static ASSETS_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/bgg-logos");

pub(crate) fn logos() -> ServeDir {
    tower_serve_static::ServeDir::new(&ASSETS_DIR)
}


#[derive(Route, Clone, Default, Serialize, Deserialize)]
#[template("/logos/index")]
pub(crate) struct Nick {}

pub(crate) fn list_route() -> String {
    Nick::axum_route()
}

pub(crate) async fn get(
    req: NestedRoute<Nick>
) -> Result<impl IntoResponse, Error> {
    Ok((
        StatusCode::OK,
        Json(ASSETS_DIR.entries().iter().filter_map(|e| e.as_file().map(|f|
            [req.nested_path.as_ref(), "logos",  f.path().to_str().expect("sensible file names")].join("/")
        )).collect::<Vec<_>>())
    ))
}
