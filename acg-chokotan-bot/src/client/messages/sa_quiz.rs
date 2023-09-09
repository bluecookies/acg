use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::http::Http;
use serenity::model::channel::Message;
use serenity::model::prelude::ChannelId;
use crate::Error;

use super::colours::*;
use super::text::*;


pub(crate) struct QuizSongMessage<H> {
    http: H,
    message: Message,
}

impl<H: AsRef<Http>> QuizSongMessage<H> {
    pub(crate) async fn new(
        http: H,
        channel_id: ChannelId,
        song_num: i64,
        loaded: bool,
    ) -> Result<Self, Error> {
        let message = channel_id.send_message(&http, quiz_song_message(song_num, loaded)).await?;
        Ok(QuizSongMessage {
            http,
            message,
        })
    }

    pub(crate) async fn set_loaded(&mut self) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            embed.colour(LOADED_COLOUR).description(LOADED_SONG_TEXT);
            self.message.edit(self.http.as_ref(), |em| em.set_embed(embed)).await?;
        }
        Ok(())
    }

    pub(crate) async fn set_playing(&mut self) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            embed.colour(PLAYING_COLOUR).description(PLAYING_TEXT);
            self.message.edit(self.http.as_ref(), |em| em.set_embed(embed)).await?;
        }
        Ok(())
    }

    pub(crate) async fn set_error(&mut self, e: Error, song_info: Option<database::SongInfo>) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            embed.colour(ERROR_COLOUR)
                .description(format!("**Error: **{}\n\n", e));
            if let Some(info) = song_info {
                embed.field("Song Name", &info.song_name, false)
                .field("Artist", &info.artist, false);
            }
            self.message.edit(self.http.as_ref(), |em| em.set_embed(embed)).await?;
        }
        Ok(())
    }


    pub(crate) async fn set_finished(mut self, song_info: Option<database::SongInfo>) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            embed.colour(FINISHED_COLOUR)
                .description("");
            if let Some(info) = song_info {
                embed
                    .field("Song Name", info.song_name, false)
                    .field("Artist", info.artist, false);
            }
            self.message.channel_id.send_message(
                self.http.as_ref(), 
                |cm| cm.embed(|em| {
                    *em = embed;
                    em
                })
            ).await?;
        }
        Ok(())
    }

}

impl<H> QuizSongMessage<H> {
    fn pop_embed(&mut self) -> Option<CreateEmbed> {
        self.message.embeds.pop().map(CreateEmbed::from)
    }
}

// messages
fn quiz_song_message<'b>(
    song_num: i64,
    loaded: bool,
) -> impl for<'a> FnOnce(&'a mut CreateMessage<'b>) -> &'a mut CreateMessage<'b> + Send
{
    move |cm| {
        cm.embed(|ce| {
            ce.title(format!("Song {}", song_num));
            if loaded {
                ce.colour(LOADED_COLOUR).description(LOADED_SONG_TEXT)
            } else {
                ce.description(LOADING_TEXT)
            }
        })
    }
}
