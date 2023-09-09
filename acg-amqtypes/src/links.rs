use serde_json::Value as JsonValue;

pub struct CatboxLinks {
    pub mp3: Option<String>,
    pub video: Option<String>,
}

impl CatboxLinks {
    pub fn is_some(&self) -> bool {
        self.mp3.is_some() || self.video.is_some()
    }
}

pub fn get_links(mut map: JsonValue) -> CatboxLinks {
    let (mut mp3, mut video) = (None, None);
    if let Some(v) = map.get_mut("catbox").map(JsonValue::take) {
        if let Some(url) = v.get("0").and_then(JsonValue::as_str) {
            if let Some((_, file)) = url.rsplit_once('/') {
                mp3 = Some(file.to_string());
            }
        }
        if let Some(url) = v.get("720").or_else(|| v.get("480")).and_then(JsonValue::as_str) {
            if let Some((_, file)) = url.rsplit_once('/') {
                video = Some(file.to_string());
            }
        }
    }
    CatboxLinks { mp3, video }
}
