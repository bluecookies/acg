use serde_json::json;
use amq_types::Friend;

use crate::{Client, Error};

impl Client {
    pub fn self_name(&self) -> Option<String> {
        let guard = self.game_data.lock().expect("poisoned mutex");
        guard.as_ref().map(|data| data.self_name.clone())
    }

    pub fn friends(&self) -> Vec<Friend> {
        let guard = self.game_data.lock().expect("poisoned mutex");
        if let Some(ref data) = &*guard {
            data.friends.clone()
        } else {
            Vec::new()
        }
    }

    pub fn answer_friend_request(&self, name: &str, accept: bool) -> Result<(), Error> {
        self.send_command("social", "friend request response", json!({"target": name, "accept": accept}))?;
        Ok(())
    }

    pub fn remove_friend(&self, name: &str) -> Result<(), Error> {
        self.send_command("social", "remove friend", json!({"target": name}))?;
        let mut guard = self.game_data.lock().expect("poisoned mutex");
        if let Some(ref mut data) = &mut *guard {
            if let Some(index) = data.friends.iter().position(|f| f.name == name) {
                data.friends.remove(index);
            }
        }
        Ok(())
    }

    pub fn set_list(&self, list_type: ListType, username: Option<&str>, setter: Option<String>) -> Result<bool, Error> {
        let mut guard = self.list_update.lock().expect("mutex poisoned");
        if guard.is_some() {
            return Ok(false);
        }
        self.send_command("library", "update anime list", json!({
            "newUsername": username.unwrap_or(""),
            "listType": list_type,
        }))?;
        *guard = Some((list_type, username.map(str::to_string), setter));
        Ok(true)
    }
}


// TODO: move this to types
// validated to only be these types
#[derive(serde::Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ListType {
    Mal,
    Anilist,
    Kitsu,
}

impl<'a> TryFrom<&'a str> for ListType {
    type Error = &'a str;
    fn try_from(value: &str) -> Result<Self, &str> {
        match value {
            "MAL" => Ok(Self::Mal),
            "ANILIST" => Ok(Self::Anilist),
            "KITSU" => Ok(Self::Kitsu),
            e => Err(e),
        }
    }
}