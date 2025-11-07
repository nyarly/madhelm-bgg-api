use mattak::routing::{extract::{ExtractedRoute as _, NestedRoute}, Route as _};
use axum::{debug_handler, extract::State, response::IntoResponse, Json};
use mattak::hypermedia::{op, ActionType, ResourceFields};
use mattak_derives::Route;
use reqwest::{Client, StatusCode};
use serde::{Serialize, Deserialize};
use sqlx::{Pool, Postgres};

use crate::{
    bgg_api::{search, SearchItem}, db::ThingData, AppState, BggLimit, Error
};


#[derive(Route, Clone, Default, Serialize, Deserialize)]
#[template("/search{?query}")]
pub(crate) struct Nick {
    query: String,
}

pub(crate) fn route() -> String {
    Nick::axum_route()
}

#[derive(Serialize)]
struct Response {
    #[serde(flatten)]
    resource_fields: ResourceFields<Nick>,
    items: Vec<SearchItem>,
    things: Vec<ThingData>
}

#[debug_handler(state = AppState)]
pub(crate) async fn get(
    State(db): State<Pool<Postgres>>,
    State(client): State<Client>,
    State(bgg_limit): State<BggLimit>,
    req: NestedRoute<Nick>
) -> Result<impl IntoResponse, Error> {
    let (items, things) = search(client, &db, req.nick.query.clone(), bgg_limit.into()).await?;

    let  response = Response{
        resource_fields: req.resource_fields(
            "api:searchThings",
            vec![ op(ActionType::View)]
        )?,
        items,
        things,
    };


    Ok((StatusCode::OK,Json(response)))
}




// XXX Comment from Wiki:
// About collections:
//
// Note that the default (or using subtype=boardgame) returns both boardgame
// and boardgameexpansion's in your collection... but incorrectly gives
// subtype=boardgame for the expansions. Workaround is to use
// excludesubtype=boardgameexpansion and make a 2nd call asking for
// subtype=boardgameexpansion
