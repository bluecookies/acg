use std::borrow::Cow;
use database::include_query;
use crate::reply::JsonReply;

#[derive(serde::Serialize)]
pub struct SongStatVintage {
    vintage: String,
    kind: String,
    guess_rate: Option<f64>,
    guess_count: i64,
    times_played: i64,
}

// TODO: clean up this type
#[derive(serde::Serialize, Default)]
pub struct SongStatDifficulty {
    diff_bin: Option<i32>,
    guess_rate: Option<f64>,
    bucket_min: Option<f32>,
    bucket_max: Option<f32>,
    guess_count: i64,
    times_played: i64,
    kind: Option<String>,
    bucket: Option<i32>,
}

pub async fn guess_rate_vintage(db: database::Database) -> JsonReply {
    db.get_stats(
        include_query!("guess_rate_vintage.sql"),
        "guess rate vintage",
        |r| SongStatVintage {
            vintage: r.get(0),
            kind: r.get(1),
            guess_rate: r.get(2),
            guess_count: r.get(3),
            times_played: r.get(4),
        },
        &[],
    ).await.into()
}

// TODO: clean this up
pub async fn guess_rate_difficulty(
    db: database::Database,
    num_bins: u32,
    kind: i32,
) -> JsonReply {
    if num_bins > 1000 {
        return JsonReply::BadRequest(Cow::Borrowed("Max bins allowed is 1000"));
    }

    let params: &[&(dyn database::ToSql + Sync)] = &[&(num_bins as i32)];

    match kind {
        0 => db.get_stats(
                include_query!("guess_rate_difficulty.sql"),
                "guess rate difficulty",
                |r| SongStatDifficulty {
                    diff_bin: r.get(0),
                    guess_rate: r.get(1),
                    guess_count: r.get(2),
                    times_played: r.get(3),
                    ..Default::default()
                },
                params,
            ).await,
        _ => db.get_stats(
                include_query!("guess_rate_difficulty_ntile.sql"),
                "guess rate difficulty ntile",
                |r| SongStatDifficulty {
                    guess_rate: r.get(0),
                    bucket_min: r.get(1),
                    bucket_max: r.get(2),
                    guess_count: r.get(3),
                    kind: r.get(4),
                    bucket: r.get(5),
                    ..Default::default()
                },
                params
            ).await,
    }.into()
}
