use chrono::{DateTime,Utc};
use serde::{Serialize};

use sqlx::{query, query_as, query_scalar, Acquire, Executor, Postgres};
use mattak::querymapping::{NoId, Error};
use mattak_derives::id_type;
// use tracing::debug;

id_type!(ThingId(i32),IdForThing);

#[derive(sqlx::FromRow, Default, Debug, Clone, Serialize)]
pub(crate) struct ThingData {
    pub bgg_id: String,
    pub kind: String,
    pub thumbnail: Option<String>,
    pub image: Option<String>,
    pub name: Option<String>,
    pub altnames: Vec<String>,
    pub description: Option<String>,
    pub year_published: Option<i32>,
    pub min_players: Option<i32>,
    pub max_players: Option<i32>,
    pub min_duration: Option<i32>,
    pub max_duration: Option<i32>,
    pub duration: Option<i32>,
}

#[derive(sqlx::FromRow, Default, Debug, Clone, Serialize)]
pub(crate) struct ThingLinks {
    pub categories: Vec<LinkData>,
    pub families: Vec<LinkData>,
    pub designers: Vec<LinkData>,
    pub publishers: Vec<LinkData>,
}

#[derive(sqlx::FromRow, Default, Debug, Clone, Serialize)]
#[allow(dead_code)] // Have to match DB
pub(crate) struct BggThing<ID: IdForThing> {
    pub id: ID,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub retreived_at: DateTime<Utc>,

    #[sqlx(flatten)]
    pub data: ThingData,

    #[sqlx(flatten)]
    pub links: ThingLinks

}

impl<T: IdForThing> BggThing<T> {
    pub fn into_data(self) -> ThingData {
        self.data
    }
}

impl BggThing<NoId> {

    pub async fn add_new<'a, DB>(&self, db: DB)
    -> Result<ThingId, Error>
where DB: Acquire<'a, Database = Postgres> + 'a {
        let mut tx = db.begin().await?;
        let data = &self.data;
        let id = query_scalar!(
            r#"insert into bgg_thing (
    "bgg_id", "kind", "name", "description", "thumbnail", "image",
    "year_published", "min_players", "max_players", "min_duration", "max_duration", "duration"
    ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
    on conflict do nothing
    returning id"#,
            data.bgg_id, data.kind, data.name, data.description, data.thumbnail, data.image,
            data.year_published, data.min_players, data.max_players, data.min_duration, data.max_duration, data.duration,
        ).fetch_one(&mut *tx)
        .await?;

        query!(
            r#"insert into bgg_altname ("thing_id", "name")
    select $1, name from unnest($2::text[]) as a(name) on conflict do nothing"#,
            id, &self.data.altnames
        ).execute(&mut *tx).await?;


        let cis = self.links.categories.iter().map(|c| c.bgg_id.clone()).collect::<Vec<_>>();
        let cns = self.links.categories.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        let category_ids = query_scalar!(
            r#"insert into bgg_category ("bgg_id", "name")
   select bgg_id, name from unnest($1::text[], $2::text[]) as a(bgg_id, name)
   on conflict do nothing
   returning id"#,
            &cis, &cns
        ).fetch_all(&mut *tx)
        .await?;

        query!(
            r#"insert into thing_category ("thing_id", "category_id")
  select $1, category_id from unnest($2::integer[]) as a(category_id)
  on conflict do nothing
"#, id, &category_ids).execute(&mut *tx).await?;

        let fis = self.links.families.iter().map(|c| c.bgg_id.clone()).collect::<Vec<_>>();
        let fns = self.links.families.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        let family_ids = query_scalar!(
            r#"insert into bgg_family ("bgg_id", "name")
   select bgg_id, name from unnest($1::text[], $2::text[]) as a(bgg_id, name)
   on conflict do nothing
   returning id"#,
            &fis, &fns
        ).fetch_all(&mut *tx)
        .await?;

        query!(
            r#"insert into thing_family ("thing_id", "family_id")
  select $1, family_id from unnest($2::integer[]) as a(family_id)
  on conflict do nothing
"#, id, &family_ids).execute(&mut *tx).await?;

        let dis = self.links.designers.iter().map(|c| c.bgg_id.clone()).collect::<Vec<_>>();
        let dns = self.links.designers.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        let designer_ids = query_scalar!(
            r#"insert into bgg_designer ("bgg_id", "name")
   select bgg_id, name from unnest($1::text[], $2::text[]) as a(bgg_id, name)
   on conflict do nothing
   returning id"#,
            &dis, &dns
        ).fetch_all(&mut *tx)
        .await?;

        query!(
            r#"insert into thing_designer ("thing_id", "designer_id")
  select $1, designer_id from unnest($2::integer[]) as a(designer_id)
  on conflict do nothing
"#, id, &designer_ids).execute(&mut *tx).await?;

        let pis = self.links.publishers.iter().map(|c| c.bgg_id.clone()).collect::<Vec<_>>();
        let pns = self.links.publishers.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        let publisher_ids = query_scalar!(
            r#"insert into bgg_publisher ("bgg_id", "name")
   select bgg_id, name from unnest($1::text[], $2::text[]) as a(bgg_id, name)
   on conflict do nothing
   returning id"#,
            &pis, &pns
        ).fetch_all(&mut *tx)
        .await?;

        query!(
            r#"insert into thing_publisher ("thing_id", "publisher_id")
  select $1, publisher_id from unnest($2::integer[]) as a(publisher_id)
  on conflict do nothing
"#, id, &publisher_ids).execute(&mut *tx).await?;

        tx.commit().await?;

        Ok((id as i32).into())
    }
}

impl BggThing<ThingId> {

    const MAX_IDS: usize = 1000;

    pub async fn get_for_bgg_ids<'a, DB>(db: DB, bgg_ids: Vec<String>) -> Result<Vec<Self>, Error>
where DB: Executor<'a, Database = Postgres> + Copy + 'a {
        let mut list = Vec::with_capacity(bgg_ids.len());

        let mut ids_iter = bgg_ids.into_iter();

        loop {
            let batch_ids = ids_iter.by_ref().take(Self::MAX_IDS).collect::<Vec<String>>();
            if batch_ids.len() == 0 {
                break
            }

            let batch = query_as(
                r#"
            with N as (
                select thing_id as id, array_agg(name) as names
                from bgg_altname group by thing_id
            ),
            C as (
                select TC.thing_id as id, array_agg((C.bgg_id, C.name)::link) as links
                from thing_category TC left join bgg_category C on C.id = TC.category_id
                group by TC.thing_id
            ),
            F as (
                select TF.thing_id as id, array_agg((F.bgg_id, F.name)::link) as links
                from thing_family TF left join bgg_family F on F.id = TF.family_id
                group by TF.thing_id
            ),
            D as (
                select TD.thing_id as id, array_agg((D.bgg_id, D.name)::link) as links
                from thing_designer TD left join bgg_designer D on D.id = TD.designer_id
                group by TD.thing_id
            ),
            P as (
                select TP.thing_id as id, array_agg((P.bgg_id, P.name)::link) as links
                from thing_publisher TP left join bgg_publisher P on P.id = TP.publisher_id
                group by TP.thing_id
            )
            select
                T.*,
                coalesce(N.names, array[]::text[]) as "altnames",
                coalesce(C.links, array[]::link[]) as "categories",
                coalesce(F.links, array[]::link[]) as "families",
                coalesce(D.links, array[]::link[]) as "designers",
                coalesce(P.links, array[]::link[]) as "publishers"
            from bgg_thing T
            left join N on N.id = T.id
            left join C on C.id = T.id
            left join F on F.id = T.id
            left join D on D.id = T.id
            left join P on P.id = T.id
            where T.bgg_id = any($1)
            "#)
                .bind(&batch_ids)
                .fetch_all(db)
            .await?;

            list.extend(batch);
        }

        Ok(list)
    }
}
// n-n with
//   boardgamemechanic (hi Sarah!)
//   boardgamefamily /family
//   boardgameexpansion /thing
//   boardgameaccessory /thing
//   boardgamecompilation /thing
//   boardgamedesigner 100% IDK
//   boardgameartist 100% IDK
//   boardgamepublisher 100% IDK
//
#[derive(Default, Serialize, Debug, sqlx::Type, Clone)]
#[sqlx(type_name = "link")]
pub(crate) struct LinkData {
    pub bgg_id: String,
    pub name: String,
}

id_type!(CategoryId(i32),IdForCategory);

#[derive(Default, Serialize, Debug, Clone, sqlx::FromRow)]
pub(crate) struct BggCategory<ID: IdForCategory> {
    pub id: ID,
    pub created_at: DateTime<Utc>,

    #[sqlx(flatten)]
    pub data: LinkData
}

id_type!(FamilyId(i32),IdForFamily);

#[derive(Default, Serialize, Debug, sqlx::FromRow, Clone)]
pub(crate) struct BggFamily<ID: IdForFamily> {
    pub id: ID,
    pub created_at: DateTime<Utc>,

    #[sqlx(flatten)]
    pub data: LinkData
}

id_type!(DesignerId(i32), IdForDesigner);

#[derive(Default, Serialize, Debug, sqlx::FromRow, Clone)]
pub(crate) struct BggDesigner<ID: IdForDesigner> {
    pub id: ID,
    pub created_at: DateTime<Utc>,

    #[sqlx(flatten)]
    pub data: LinkData
}

id_type!(PublisherId(i32), IdForPublisher);

#[derive(Default, Serialize, Debug, sqlx::FromRow, Clone)]
pub(crate) struct BggPublisher<ID: IdForPublisher> {
    pub id: ID,
    pub created_at: DateTime<Utc>,

    #[sqlx(flatten)]
    pub data: LinkData
}
