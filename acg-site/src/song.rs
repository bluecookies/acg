use crate::reply::JsonReply;

pub async fn update_song_data(data: database::SongData, db: database::Database) -> JsonReply {
    if data.player_name.is_none() {
        JsonReply::BadRequest("no player_name provided".into())
    } else {
        // TODO: map user auth error to bad request
        db.update_song_data(data).await.into()
    }
}

pub async fn search_song(query: database::SearchQuery, db: database::Database) -> JsonReply {
    if query.search.is_empty() {
        JsonReply::BadRequest("empty search string".into())
    } else {
        db.search_songs(query).await.into()
    }
}

pub async fn query_song(song_id: i32, db: database::Database) -> JsonReply {
    db.query_song(song_id).await.into()
}
