use core::fmt;
use serde_json::Value;
use std::fmt::Formatter;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizSongInfo {
    pub play_length: f64,
    pub playback_speed: f64,
    #[serde(alias = "startPont")]
    pub start_point: f64,
    pub video_info: Option<VideoInfo>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub id: i64,
    pub video_map: serde_json::Value,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySongInfo {
    pub extra_guess_time: f64,
    // pub multiple_choice_names: Option<_>,
    pub on_last_song: bool,
    // progress_bar_state(length, played)
    pub song_number: i64,
    pub time: f64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    // "atEveryone":false,
    // "badges":[],
    // "emojis":{"customEmojis":[],"emotes":[]},
    // "messageId":14,
    // "modMessage":false,
    // "nameColor":0,
    // "nameGlow":0,
    // "teamMessage":false
    pub message: String,
    pub sender: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectateGameData {
    pub quiz_state: Option<QuizState>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizState {
    pub next_video_info: QuizSongInfo,
    pub song_info: QuizSongInfo,
    pub song_number: i64,
    pub song_timer: f64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerResults {
    // group_map: Vec<Vec<PlayerId>>
    // players: Vec<PlayerData>
    //  correct, game_player_id, level, pose, position, position_slot, score
    pub song_info: ResultsSongInfo,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultsSongInfo {
    pub anime_names: AnimeNames,
    #[serde(default)]
    pub alt_anime_names: Vec<String>,
    // pub alt_anime_names_answers: Vec<String>,
    pub ann_id: i64,
    pub site_ids: SiteIds,
    #[serde(deserialize_with = "deserialize_difficulty")]
    pub anime_difficulty: Option<f64>,
    pub anime_type: String,
    pub anime_score: f64,
    pub vintage: String,
    #[serde(default)]
    pub anime_tags: Vec<String>,
    #[serde(default)]
    pub anime_genre: Vec<String>,

    pub song_name: String,
    pub artist: String,
    #[serde(rename = "type")]
    pub ty: SongType,
    pub type_number: Option<i64>,
    pub video_target_map: serde_json::Value,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimeNames {
    pub english: String,
    pub romaji: String,
}

#[derive(serde_repr::Deserialize_repr)]
#[repr(i64)]
pub enum SongType {
    Opening = 1,
    Ending = 2,
    Insert = 3,
    Unknown,
}

impl fmt::Display for SongType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Opening => write!(f, "Opening"),
            Self::Ending => write!(f, "Ending"),
            Self::Insert => write!(f, "Insert Song"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteIds {
    pub mal_id: Option<i64>,
    pub ani_list_id: Option<i64>,
    pub kitsu_id: Option<i64>,
}

fn deserialize_difficulty<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f64>, D::Error> {
    let value: Value = serde::Deserialize::deserialize(deserializer)?;
    match value {
        Value::String(s) => {
            if s == "Unrated" {
                return Ok(None);
            }
            s.parse::<f64>()
                .map(Option::Some)
                .map_err(|e| serde::de::Error::custom(format!("invalid difficulty string: {}", e)))
        }
        Value::Number(x) => Ok(x.as_f64()),
        Value::Null => Ok(None),
        v => Err(serde::de::Error::custom(format!(
            "invalid difficulty type: {}",
            v
        ))),
    }
}
