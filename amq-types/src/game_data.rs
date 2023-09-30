#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameData {
    #[serde(rename = "self")]
    pub self_name: String,
    pub friends: Vec<Friend>,

    // list stuff
    pub ani_list: Option<String>,
    pub ani_list_last_update: Option<String>,
    pub kitsu: Option<String>,
    pub kitsu_last_update: Option<String>,
    pub mal_name: Option<String>,
    pub mal_last_update: Option<String>,
    // genreInfo, tagInfo
    // canReconnectGame, savedSettings
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub avatar_name: String,
    //"avatarProfileImage": Null,
    pub color_name: String,
    // "game_state": Option<GameState>,
    pub name: String,
    pub online: bool,
    // "optionActive": Number(1),
    pub option_name: String,
    pub outfit_name: String,
    // "outfitName"
    // "profileEmoteId"
    // "status"
}
