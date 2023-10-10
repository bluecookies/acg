use std::collections::HashMap;

use crate::types;
use crate::Error::QueryError;
use crate::{Database, Error};
use tokio_postgres::types::ToSql;
use tokio_postgres::Row;

// song play data received from amq
#[derive(serde::Deserialize)]
pub struct SongData {
    pub correct: Option<bool>,
    pub player_name: Option<String>,
    pub video_length: Option<f64>,
    #[serde(flatten)]
    pub song_info: amq_types::ResultsSongInfo,
}

#[derive(Clone)]
pub struct SongInfo {
    pub song_name: String,
    pub artist: String,
    // map of column name to value
    pub fields: HashMap<Box<str>, types::Value>,
}

impl SongInfo {
    // this can eventually be changed for different types of urls
    pub fn url(&self) -> Option<&str> {
        if let Some(types::Value::String(ref s)) = self.fields.get("url") {
            Some(s.as_ref())
        } else {
            None
        }
    }
}

impl SongInfo {
    // this cannot fail since it is not a user defined query
    fn from_amq_url(r: Row, url: Option<String>) -> Self {
        let mut fields = HashMap::new();
        if let Some(url) = url {
            fields.insert(Box::from("url"), types::Value::String(url));
        }
        SongInfo {
            song_name: r.get(0),
            artist: r.get(1),
            fields,
        }
    }

    fn from_query(r: Row, types: &HashMap<Box<str>, types::ValueType>) -> Result<Self, Error> {
        use types::{Value, ValueType};
        let mut info = SongInfo {
            song_name: String::new(),
            artist: String::new(),
            fields: HashMap::new(),
        };
        for col in r.columns() {
            let colname = col.name();
            if colname == "songname" {
                info.song_name = r.try_get(colname).map_err(Error::TypeError)?;
            } else if colname == "artist" {
                info.artist = r.try_get(colname).map_err(Error::TypeError)?;
            } else if colname == "url" {
                let value = Value::String(r.try_get(colname).map_err(Error::TypeError)?);
                info.fields.insert(colname.into(), value);
            } else {
                let value = match types.get(colname).copied() {
                    Some(ValueType::String) => {
                        Value::String(r.try_get(colname).map_err(Error::TypeError)?)
                    }
                    Some(ValueType::Bool) => {
                        Value::Bool(r.try_get(colname).map_err(Error::TypeError)?)
                    }
                    Some(ValueType::I32) => Value::Integer(
                        r.try_get::<_, i32>(colname).map_err(Error::TypeError)? as i64,
                    ),
                    Some(ValueType::I64) => {
                        Value::Integer(r.try_get::<_, i64>(colname).map_err(Error::TypeError)?)
                    }
                    Some(ValueType::F32) => {
                        Value::Float(r.try_get::<_, f32>(colname).map_err(Error::TypeError)? as f64)
                    }
                    Some(ValueType::F64) => {
                        Value::Float(r.try_get::<_, f64>(colname).map_err(Error::TypeError)?)
                    }
                    None => continue,
                };
                info.fields.insert(colname.into(), value);
            }
        }
        Ok(info)
    }
}

#[derive(serde::Deserialize)]
pub struct SearchQuery {
    pub search: String,
    #[serde(default)]
    pub exact: bool,
}

#[derive(serde_tuple::Serialize_tuple, serde_tuple::Deserialize_tuple)]
pub struct SearchResult {
    song_id: i32,
    song_name: String,
    artist: String,
    romaji: String,
    difficulty: Option<f32>,
}

impl Database {
    // Updates the anime data (including name and tags),
    //  then the song data and finally the song play data
    //  for an AMQ song.
    // This function can be called without any of the the player song guess data
    //  so if it is being called from an external source, it should be checked
    //  to exist before calling this
    pub async fn update_song_data(&self, mut data: SongData) -> Result<(), Error> {
        // my timezone
        let date = chrono::offset::Utc::now().with_timezone(&chrono_tz::Australia::Sydney);
        let date = date.naive_local(); // remove timezone info to give to postgres

        let mut client = self.client().await?;
        let transaction = client.transaction().await?;

        // Insert/Update anime data
        let params: &[&(dyn ToSql + Sync)] = &[
            // ann_id,
            &(data.song_info.ann_id as i32),
            // romaji, english,
            &data.song_info.anime_names.romaji,
            &data.song_info.anime_names.english,
            // mal_id, anilist_id, kitsu_id,
            &data.song_info.site_ids.mal_id.map(|x| x as i32),
            &data.song_info.site_ids.ani_list_id.map(|x| x as i32),
            &data.song_info.site_ids.kitsu_id.map(|x| x as i32),
            // type, score, vintage,
            &data.song_info.anime_type,
            &(data.song_info.anime_score as f32),
            &data.song_info.vintage,
            // created_date,
            &date,
        ];
        let statement = prepare_statement!(transaction, "pg_insert_anime.sql", "update anime")?;
        transaction
            .execute(&statement, params)
            .await
            .map_err(|e| QueryError("update anime", e))?;

        // Insert/Update anime tags (1 for genre, 2 for tags)
        let tags_iter = data
            .song_info
            .anime_genre
            .iter()
            .map(|g| (1, g))
            .chain(data.song_info.anime_tags.iter().map(|t| (2, t)));

        let statement =
            prepare_statement!(transaction, "pg_insert_anime_tags.sql", "update anime tags")?;
        for (tag_type, tag) in tags_iter {
            let params: &[&(dyn ToSql + Sync)] = &[
                // amq_anime_id,
                &(data.song_info.ann_id as i32),
                // tag_type, tag,
                &tag_type,
                tag,
                // modified_date
                &date,
            ];
            transaction
                .execute(&statement, params)
                .await
                .map_err(|e| QueryError("update anime tags", e))?;
        }
        // Insert/Update anime names
        let names_iter = data
            .song_info
            .alt_anime_names
            .iter()
            .chain(std::iter::once(&data.song_info.anime_names.romaji))
            .chain(std::iter::once(&data.song_info.anime_names.english));

        let statement = prepare_statement!(
            transaction,
            "pg_insert_anime_names.sql",
            "update anime names"
        )?;
        for name in names_iter {
            let params: &[&(dyn ToSql + Sync)] = &[
                // ann_id,
                &(data.song_info.ann_id as i32),
                // name
                name,
                // modified_date
                &date,
            ];
            transaction
                .execute(&statement, params)
                .await
                .map_err(|e| QueryError("update anime names", e))?;
        }
        // Insert/Update song data
        // TODO: change db schema to split type and type number
        let song_type = if let Some(num) = data.song_info.type_number {
            format!("{} {}", &data.song_info.ty, num)
        } else {
            data.song_info.ty.to_string()
        };
        let links = amq_types::get_links(data.song_info.video_target_map.take());
        let params: &[&(dyn ToSql + Sync)] = &[
            // songname, artist,
            &data.song_info.song_name,
            &data.song_info.artist,
            // anime_id,
            &(data.song_info.ann_id as i32),
            // type,
            &song_type,
            // mp3, video,
            &links.mp3,
            &links.video,
            // video_length,
            &data.video_length.map(|x| x as f32),
            // difficulty,
            &data.song_info.anime_difficulty.map(|x| x as f32),
            // created_date,
            &date,
        ];

        let statement = prepare_statement!(transaction, "pg_update_song.sql", "update song data")?;
        let res = transaction
            .query_opt(&statement, params)
            .await
            .map_err(|e| QueryError("update song data", e))?;
        let song_id: i32 = if let Some(row) = res {
            row.get(0)
        } else {
            let statement =
                prepare_statement!(transaction, "pg_insert_song.sql", "insert song data")?;
            // we use the same params here as the update
            let row = transaction
                .query_one(&statement, params)
                .await
                .map_err(|e| QueryError("insert song data", e))?;
            row.get(0)
        };
        // Insert/Update song plays
        // only do this if player name is set
        if let Some(ref player_name) = data.player_name {
            // ~~get the player id - could cache this?~~
            // player id get in query
            let params: &[&(dyn ToSql + Sync)] = &[
                // song_id, name, correct, date
                &song_id,
                player_name,
                &data.correct,
                &date,
            ];
            // TODO: handle failure here (e.g. if player name invalid)
            let statement = prepare_statement!(
                transaction,
                "pg_insert_song_plays.sql",
                "update song plays",
                INT4,
                TEXT,
                BOOL,
                TIMESTAMP,
            )?;
            let row = transaction
                .query_one(&statement, params)
                .await
                .map_err(|e| QueryError("update song plays", e))?;
            // couldn't figure out a good way to update the guess rate in the same query
            // but just update it if we had a guess
            if data.correct.is_some() {
                let player_id: i32 = row.get(0);
                let params: &[&(dyn ToSql + Sync)] = &[&song_id, &player_id];
                let statement = prepare_statement!(
                    transaction,
                    "pg_update_guess_rate.sql",
                    "update song guess rate"
                )?;
                transaction
                    .execute(&statement, params)
                    .await
                    .map_err(|e| QueryError("update song guess rate", e))?;
            }
        }
        transaction.commit().await?;
        Ok(())
    }

    // Returns the song information given the AMQ Catbox links used
    pub async fn get_song_info(
        &self,
        links: amq_types::CatboxLinks,
    ) -> Result<Option<SongInfo>, Error> {
        let client = self.client().await?;

        let statement = prepare_statement!(client, "get_song_from_url.sql", "get song info")?;
        let params: &[&(dyn ToSql + Sync)] = &[&links.mp3, &links.video];
        let row = client
            .query_opt(&statement, params)
            .await
            .map_err(|e| QueryError("get song", e))?;
        let url = links.mp3.or(links.video);
        let info = row.map(|r| SongInfo::from_amq_url(r, url));
        Ok(info)
    }

    pub async fn search_songs(&self, search: SearchQuery) -> Result<Vec<SearchResult>, Error> {
        let client = self.client().await?;

        let statement = prepare_statement!(client, "search_songs.sql", "search songs")?;
        let params: &[&(dyn ToSql + Sync)] = &[&search.search, &search.exact];
        let rows = client
            .query(&statement, params)
            .await
            .map_err(|e| QueryError("search songs", e))?;
        let results = rows
            .into_iter()
            .map(|r| SearchResult {
                song_id: r.get(0),
                song_name: r.get(1),
                artist: r.get(2),
                romaji: r.get(3),
                difficulty: r.get(4),
            })
            .collect();
        Ok(results)
    }

    pub async fn query_song(&self, song_id: i32) -> Result<Vec<(String, String)>, Error> {
        let client = self.client().await?;

        let statement = prepare_statement!(client, "query_song.sql", "query song")?;
        let params: &[&(dyn ToSql + Sync)] = &[&song_id];
        let rows = client
            .query(&statement, params)
            .await
            .map_err(|e| QueryError("query song", e))?;
        let results = rows.into_iter().map(|r| (r.get(0), r.get(1))).collect();
        Ok(results)
    }
}

impl Database {
    // query: sql query string
    // params: parameters to bind to the sql query
    // types: map from column names to the types to be read as
    pub async fn fetch_song_info(
        &self,
        query: &str,
        params: &[Box<dyn types::SqlValue>],
        types: &HashMap<Box<str>, types::ValueType>,
    ) -> Result<Vec<SongInfo>, Error> {
        let client = self.client().await?;
        let statement = prepare_statement!(client, query, "get quiz song")?;

        let params: Vec<_> = params
            .iter()
            .map(|p| types::SqlValue::to_sql(&**p))
            .collect();
        let row = client
            .query(&statement, &params)
            .await
            .map_err(|e| QueryError("fetch quiz song info", e))?;

        let info = row
            .into_iter()
            .map(|r| SongInfo::from_query(r, types))
            .collect::<Result<_, _>>()?;

        Ok(info)
    }
}
