use std::time::Duration;

use bounded_join_set::JoinSet;
use mattak::querymapping::NoId;
use quick_xml::{events::{BytesStart, Event}, name::QName, Reader};
use reqwest::{Client, StatusCode};
use serde::Serialize;
use sqlx::{Pool, Postgres};
use tokio::time::sleep;
use tracing::debug;

use crate::{db::{BggThing, LinkData, ThingData}, Error};

// const XMLAPI: &str = "https://boardgamegeek.com/xmlapi";
const XMLAPI2: &str = "https://boardgamegeek.com/xmlapi2";

fn string_attr (tag: &BytesStart, name: &str) -> String {
    tag.try_get_attribute(name)
        .unwrap_or(None)
        .and_then(|a| a.unescape_value().ok())
        .unwrap_or_default()
        .to_string()
}

const BGG_THING_BATCH_SIZE: usize = 20;

#[derive(Default, Serialize)]
pub(crate) struct SearchItem {
    id: String,
    kind: String
}

pub(crate) async fn search(client: Client, db: &Pool<Postgres>, query: String, bgg_limit: usize) -> Result<(Vec<SearchItem>, Vec<ThingData>), Error>
{
    let url = format!("{XMLAPI2}/search?query={}", query);
    let rz = client.get(url).send().await?;

    let text = rz.text().await?;

    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);

    let mut items = vec![];

    loop {
        match reader.read_event().unwrap() {
            Event::Eof => break,
            Event::Start(tag) if tag.local_name().as_ref() == "item".as_bytes() => {
                let id = string_attr(&tag, "id");
                let kind = string_attr(&tag, "type");
                items.push(SearchItem{id, kind});
            },
            _ => ()
        }
    }

    let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    let mut things: Vec<_> = BggThing::get_for_bgg_ids(db, ids.clone())
        .await
        .map_err(mattak::Error::from)?
        .iter()
        .map(|record| record.data.clone())
        .collect();
    debug!("cached: {:?}", things);
    let needed_ids = ids.iter().filter(|id| {
        let check = (*id).clone();
        !things.iter().any(
            |item| item.bgg_id == *check)
    }).cloned().collect::<Vec<_>>();
    debug!("needed_ids: {needed_ids:?}");

    let mut batch_index = 0;

    let batch_iter = std::iter::from_fn(move || {
        if batch_index >= needed_ids.len() {
            None
        } else {
            let top = std::cmp::min(batch_index + BGG_THING_BATCH_SIZE, needed_ids.len());
            let batch = &needed_ids[batch_index..top];
            batch_index += BGG_THING_BATCH_SIZE;
            Some(Vec::from(batch))
        }
    });

    let mut fetchset = JoinSet::new(bgg_limit);
    for (count, id_batch) in batch_iter.enumerate() {
        let our_client = client.clone();
        let our_db = db.clone();
        debug!("spawning fetch job {count}: {id_batch:?}");
        fetchset.spawn(async move {
            match fetch_things(our_client, our_db, id_batch).await {
                Ok(thing) => Some(thing),
                Err(err) => {
                    debug!("error fetching Thing: {err:?}");
                    None
                },
            }
        });
        debug!("spawned  fetch job {count}");
    }
    debug!("spawned all fetch jobs");

    while let Some(maybe_things) = fetchset.join_next().await {
        if let Ok(Some(fetched_things)) = maybe_things {
            for thing in fetched_things {
                things.push(thing.data.clone());
            }
        }
    }

    Ok((items, things))

}


pub(crate) async fn fetch_things(client: Client, db: Pool<Postgres>, bgg_ids: Vec<String>) -> Result<Vec<BggThing<NoId>>, Error> {
    debug!("ID: {bgg_ids:?} Fetching thing data");
    let url = format!("{XMLAPI2}/thing?id={}", bgg_ids.join(","));
    let mut pause = Duration::from_millis(500);
    let maxwait = Duration::from_secs(30);

    let rz = loop {
        let rz = client.get(&url).send().await?;
        let status = rz.status();
        debug!("ID: {bgg_ids:?} Response status: {status:?}");
        debug!("ID: {bgg_ids:?} Response headers: {:?}", rz.headers());
        if status.is_success() {
            break rz;
        }
        if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            debug!("ID: {bgg_ids:?} Response body: {}", rz.text().await?);
            debug!("ID {bgg_ids:?} Waiting {pause:?} and retrying");
            sleep(pause).await;
            pause = pause.mul_f32(rand::random::<f32>() + 1.5 );
            debug!("ID {bgg_ids:?} next retry will be {pause:?}");
            continue;
        }
        if pause > maxwait {
            debug!("ID {bgg_ids:?} new wait would be {pause:?}, giving up");
            return Err(Error::GivingUp(status));
        }
        return Err(Error::Upstream(status));
    };

    let text = rz.text().await?;

    let mut items = Vec::<BggThing::<NoId>>::new();
    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event().unwrap() {
            Event::Start(tag) if tag.local_name().as_ref() == b"items" => break,
            Event::Start(tag) => {
                debug!("ignoring tag: {tag:?}");
                reader.read_to_end(tag.to_end().into_owned().name())?;
            }
            ev@(Event::Empty(_) | Event::Text(_) | Event::CData(_) | Event::Comment(_) | Event::Decl(_)) => {
                debug!("ignoring {ev:?}");
            }
            ev => {
                debug!("ick! {ev:?}");
                return Err(Error::MalformedResponse)
            }
        }
    }
    loop {
        match reader.read_event().unwrap() {
            Event::Eof => return Err(Error::MalformedResponse),
            Event::Start(tag) if tag.local_name().as_ref() == "item".as_bytes() => {
                let id = string_attr(&tag, "id");
                let kind = string_attr(&tag, "type");
                let item = BggThing::extract_xml(&mut reader, id, kind, tag.to_end().into_owned().name())?;
                match item.add_new(&db).await {
                    Ok(_) => (),
                    Err(err) => {
                        debug!("error storing Thing: {err:?}");
                    }
                }
                //reader.read_to_end(tag.to_end().into_owned().name())?;
                debug!("item: {item:?}");
                items.push(item);
            },
            Event::Start(tag) => {
                debug!("OPEN {tag:?}");
                reader.read_to_end(tag.to_end().into_owned().name())?;
            },
            Event::End(tag) if tag.local_name().as_ref() == b"items" => {
                break
            },
            Event::End(tag) => {
                debug!("CLSE {tag:?}");
            },
            Event::Empty(tag) => debug!("EMTY {tag:?}"),
            Event::Text(text) => debug!("TEXT {text:?}"),
            _ => ()
        }
    };

    debug!("ID {bgg_ids:?} fetched: {}", items.len());

    Ok(items)
}

impl BggThing<NoId> {
    pub fn extract_xml(reader: &mut Reader<&[u8]>, bgg_id: String, kind: String, until: QName<'_>) -> Result<BggThing<NoId>, Error> {
        use quick_xml::events::Event;
        let data = ThingData{bgg_id, kind, ..Default::default()};
        let mut item = BggThing{data, ..Default::default()};
        loop {
            match reader.read_event().unwrap() {
                Event::Eof => break, //XXX error?
                Event::Start(tag) => {
                    match tag.name().as_ref() {
                        b"thumbnail" => {
                            let th = reader.read_text(tag.to_end().into_owned().name())?;
                            item.data.thumbnail = Some(th.into());
                        },
                        b"image" => {
                            let img = reader.read_text(tag.to_end().into_owned().name())?;
                            item.data.image = Some(img.into());
                        },
                        b"description" => {
                            let description = reader.read_text(tag.to_end().into_owned().name())?;
                            item.data.description = Some(description.into());
                        }
                        _ => {
                            debug!("ignoring tag: {tag:?}");
                            reader.read_to_end(tag.to_end().into_owned().name())?;
                        }
                    }
                }
                Event::Empty(tag) => {
                    match tag.name().as_ref() {
                        b"name" => {
                            let ty = string_attr(&tag, "type");
                            match ty.as_ref() {
                                "primary" => item.data.name = Some(string_attr(&tag, "value")),
                                "alternate" => item.data.altnames.push(string_attr(&tag, "value")),
                                _ => debug!("unknown item name type: {ty}")
                            }
                        }
                        b"yearpublished" => {
                            item.data.year_published = Some(string_attr(&tag, "value").parse()?);
                        }
                        b"minplaytime" => {
                            item.data.min_duration = Some(string_attr(&tag, "value").parse()?);
                        }
                        b"maxplaytime" => {
                            item.data.max_duration = Some(string_attr(&tag, "value").parse()?);
                        }
                        b"playingtime" => {
                            item.data.duration = Some(string_attr(&tag, "value").parse()?);
                        }
                        b"minplayers" => {
                            item.data.min_players = Some(string_attr(&tag, "value").parse()?);
                        }
                        b"maxplayers" => {
                            item.data.max_players = Some(string_attr(&tag, "value").parse()?);
                        }
                        b"link" => {
                            match string_attr(&tag, "type").as_ref() {
                                "boardgamecategory" => {
                                    let bgg_id = string_attr(&tag, "id");
                                    let name = string_attr(&tag, "value");
                                    item.links.categories.push(LinkData { bgg_id, name });
                                }
                                "boardgamefamily" => {
                                    let bgg_id = string_attr(&tag, "id");
                                    let name = string_attr(&tag, "value");
                                    item.links.families.push(LinkData { bgg_id, name });
                                }
                                "boardgamedesigner" => {
                                    let bgg_id = string_attr(&tag, "id");
                                    let name = string_attr(&tag, "value");
                                    item.links.designers.push(LinkData { bgg_id, name });
                                }
                                "boardgamepublisher" => {
                                    let bgg_id = string_attr(&tag, "id");
                                    let name = string_attr(&tag, "value");
                                    item.links.publishers.push(LinkData { bgg_id, name });
                                }
                                ty => debug!("ignoring unknown link type: {}", ty.to_string())
                                /*
                                * Some of this list (and some of the above)
                                * should be "tags" instead of links
                                * criteria to consider:
                                * BGG's API doesn't have data for it (i.e. we can't get a list of
                                * games from the link)
                                * The think itself doesn't have (enough, meaningful) data beyond a
                                * name and an id.
                                * A "tags" table then handles all of those.
                                *
                                */
                                // rpg
                                // rpggenre
                                // rpgseries
                                // rpgcategory
                                // rpgpublisher
                                // rpgdesigner
                                // rpgartist
                                // rpgproducer
                                // boardgamemechanic
                                // boardgameexpansion
                                // boardgamecompilation
                                // boardgameartist
                                // boardgameaccessory
                                // boardgameimplementation
                            }
                        }
                        _ => debug!("ignoring empty tag: {tag:?}")
                    }
                },
                Event::End(tag) if tag.name().as_ref() == until.as_ref() => break,
                _ => ()
            }
        }
        Ok(item)
    }
}
