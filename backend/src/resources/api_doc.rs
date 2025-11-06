use axum::{debug_handler, response::IntoResponse, Json};
use mattak::{hypermedia::op, routing::{extract::{ExtractedRoute as _, NestedRoute}, Route as _}};
use mattak_derives::Route;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{resources, AppState, Error};


#[derive(Route, Clone, Default, Serialize, Deserialize)]
#[template("/")]
pub(crate) struct Nick {}

pub(crate) fn route() -> String {
    Nick::axum_route()
}

#[debug_handler(state = AppState)]
pub(crate) async fn get(
    req: NestedRoute<Nick>
) -> Result<impl IntoResponse, Error> {
    use mattak::hypermedia::ActionType::*;


    Ok((StatusCode::OK, Json(json!({
        "root": req.affordance("root", vec![op(View)]),
        "logo_list": req
            .default_relative_route::<resources::branding::Nick>("")
            .affordance("list", vec![op(View)]),
        "search": req
            .default_relative_route::<resources::search::Nick>("")
            .affordance("search", vec![op(Find)]),
        "thing": req
            .default_relative_route::<resources::thing::Nick>("")
            .affordance("thing", vec![op(View)])
    }))))
}
