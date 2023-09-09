use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::http::Http;
use serenity::model::channel::{Message, MessageReference};
use crate::Error;

use super::colours::*;
use super::text::*;

pub(crate) struct PlaySongMessage<H> {
    http: H,
    message: Message,

    curr_time: Option<f64>,
    total_duration: Option<f64>,
    loaded: Option<f64>,
    error: Option<String>,
}

impl<H: AsRef<Http>> PlaySongMessage<H> {
    pub(crate) async fn new(
        http: H,
        msg: impl Into<MessageReference>,
        url: String,
    ) -> Result<Self, Error> {
        let msg = msg.into();
        let message = msg.channel_id.send_message(&http, play_song_info_message(msg, url)).await?;
        Ok(PlaySongMessage {
            http,
            message,

            curr_time: None,
            total_duration: None,
            loaded: None,
            error: None,
        })
    }

    pub(crate) async fn set_playing(&mut self) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            if self.error.is_none() {
                embed.colour(PLAYING_COLOUR);
            }
            embed.title(PLAYING_TEXT);
            self.message.edit(self.http.as_ref(), |em| em.set_embed(embed)).await?;
        }
        Ok(())
    }

    pub(crate) async fn set_finished(&mut self) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            embed.colour(FINISHED_COLOUR).title(FINISHED_PLAYING_TEXT);
            self.message.edit(self.http.as_ref(), |em| em.set_embed(embed)).await?;
        }
        Ok(())
    }

    pub(crate) async fn set_error(&mut self, e: impl std::fmt::Display) -> Result<(), Error> {
        self.error = Some(e.to_string());
        if let Some(mut embed) = self.pop_embed() {
            embed.colour(ERROR_COLOUR).description(self.desc());
            self.message.edit(self.http.as_ref(), |em| em.set_embed(embed)).await?;
        }
        Ok(())
    }

    pub(crate) async fn update_loaded(&mut self, time: f64) -> Result<(), Error> {
        self.loaded = Some(time);
        if let Some(mut embed) = self.pop_embed() {
            embed.footer(|cef| cef.text(self.footer_text()));
            self.message.edit(self.http.as_ref(), |em| em.set_embed(embed)).await?;
        }
        Ok(())
    }

    pub(crate) async fn update_time(&mut self, time: f64) -> Result<(), Error> {
        self.curr_time = Some(time);
        if let Some(mut embed) = self.pop_embed() {
            embed.description(self.desc());
            self.message.edit(self.http.as_ref(), |em| em.set_embed(embed)).await?;
        }
        Ok(())
    }

    pub(crate) async fn update_total_duration(&mut self, time: f64) -> Result<(), Error> {
        self.total_duration = Some(time);
        if let Some(mut embed) = self.pop_embed() {
            embed.description(self.desc());
            self.message.edit(self.http.as_ref(), |em| em.set_embed(embed)).await?;
        }
        Ok(())
    }
}

impl<H> PlaySongMessage<H> {
    fn pop_embed(&mut self) -> Option<CreateEmbed> {
        // we don't expect the message to have more than 1 embed
        //  if there are no embeds (e.g. deleted) just do nothing
        self.message.embeds.pop().map(CreateEmbed::from)
    }

    fn desc(&self) -> String {
        let err_msg = if let Some(ref e) = self.error {
            format!("**Error: **{}\n\n", e)
        } else {
            String::new()
        };
        let t = self.curr_time.unwrap_or(0.0);
        if let Some(d) = self.total_duration {
            format!("{}{}/{}", err_msg, format_time(t), format_time(d))
        } else {
            format!("{}{}", err_msg, format_time(t))
        }
    }

    fn footer_text(&self) -> String {
        if let Some(t) = self.loaded {
            format!("Loaded {:.2}s", t)
        } else {
            String::new()
        }
    }
}

fn format_time(seconds: f64) -> String {
    let seconds = seconds as u64;
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    if minutes >= 60 {
        let (hours, minutes) = (minutes / 60, minutes % 60);
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

// messages
fn play_song_info_message<'b>(
    msg_ref: MessageReference,
    url: String,
) -> impl for<'a> FnOnce(&'a mut CreateMessage<'b>) -> &'a mut CreateMessage<'b> + Send
{
    move |cm| {
        cm.reference_message(msg_ref).embed(|cm| {
            cm.title(LOADING_TEXT).url(url)
        })
    }
}
