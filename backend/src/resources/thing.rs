use axum::{debug_handler, extract::State, response::IntoResponse, Json};
use mattak::{hypermedia::{op, ActionType, ResourceFields}, routing::{extract::{ExtractedRoute, NestedRoute}, Route}};
use mattak_derives::Route;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

use crate::{bgg_api::fetch_things, db::ThingData, AppState, Error};

#[derive(Route, Clone, Default, Serialize, Deserialize)]
#[template("/thing{?id}")]
pub(crate) struct Nick {
    id: String,
}

pub(crate) fn route() -> String {
    Nick::axum_route()
}

#[derive(Serialize)]
struct Response {
    #[serde(flatten)]
    resource_fields: ResourceFields<Nick>,
    thing: ThingData
}

#[debug_handler(state = AppState)]
pub(crate) async fn get(
    State(db): State<Pool<Postgres>>,
    State(client): State<Client>,
    req: NestedRoute<Nick>
) -> Result<impl IntoResponse, Error> {
    let things = fetch_things(client, db, vec![req.nick.id.clone()]).await?;

    if let Some(thing) = things.get(0) {
        Ok((StatusCode::OK, Json(Response{
            resource_fields: req.resource_fields("api:thingDetail", vec![op(ActionType::View)])?,
            thing: thing.data.clone()
        })))
    } else {
        Err(Error::StatusCode(StatusCode::NOT_FOUND, "No thing by that ID".to_string()))
    }

}
