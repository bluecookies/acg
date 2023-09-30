use poise::{serenity_prelude as serenity, ReplyHandle};
use serenity::CreateEmbed;

use crate::{Data, Error};
type Context<'a> = poise::Context<'a, Data, Error>;

use crate::client::messages::format_time;

use super::colours::*;
use super::text::*;

pub(crate) struct PlaySongMessage<'a> {
    reply: Reply<'a>,
    data: MessageData,
}

// TODO: maybe this can go up a module?
struct Reply<'a> {
    ctx: Context<'a>,
    message: ReplyHandle<'a>,
    embed: CreateEmbed,
}

#[derive(Default)]
struct MessageData {
    curr_time: Option<f64>,
    total_duration: Option<f64>,
    loaded: Option<f64>,
    error: Option<String>,
}

impl<'a> PlaySongMessage<'a> {
    pub(crate) async fn new(ctx: Context<'a>, url: String) -> Result<PlaySongMessage<'a>, Error> {
        let embed = {
            let mut ce = CreateEmbed::default();
            ce.title(LOADING_TEXT).url(url);
            ce
        };
        let message = ctx
            .send(|cm| {
                cm.embed(|e| {
                    *e = embed.clone();
                    e
                })
            })
            .await?;
        let reply = Reply {
            ctx,
            message,
            embed,
        };
        let data = MessageData::default();
        Ok(PlaySongMessage { reply, data })
    }

    pub(crate) async fn set_playing(&mut self) -> Result<(), Error> {
        self.reply
            .edit_embed(|embed| {
                if self.data.error.is_none() {
                    embed.colour(PLAYING_COLOUR);
                }
                embed.title(PLAYING_TEXT)
            })
            .await?;
        Ok(())
    }

    pub(crate) async fn set_finished(&mut self) -> Result<(), Error> {
        self.reply
            .edit_embed(|embed| embed.colour(FINISHED_COLOUR).title(FINISHED_PLAYING_TEXT))
            .await?;
        Ok(())
    }

    pub(crate) async fn set_error(&mut self, e: impl std::fmt::Display) -> Result<(), Error> {
        self.data.error = Some(e.to_string());
        self.reply
            .edit_embed(|embed| embed.colour(ERROR_COLOUR).description(self.data.desc()))
            .await?;
        Ok(())
    }

    pub(crate) async fn update_loaded(&mut self, time: f64) -> Result<(), Error> {
        self.data.loaded = Some(time);
        self.reply
            .edit_embed(|embed| embed.footer(|cef| cef.text(self.data.footer_text())))
            .await?;
        Ok(())
    }

    pub(crate) async fn update_time(&mut self, time: f64) -> Result<(), Error> {
        let last_time = self.data.curr_time.unwrap_or(0.0) as i32;
        self.data.curr_time = Some(time);
        if (time as i32) - last_time == 0 {
            return Ok(());
        }
        self.reply
            .edit_embed(|embed| embed.description(self.data.desc()))
            .await?;
        Ok(())
    }

    pub(crate) async fn update_total_duration(&mut self, time: f64) -> Result<(), Error> {
        self.data.total_duration = Some(time);
        self.reply
            .edit_embed(|embed| embed.description(self.data.desc()))
            .await?;
        Ok(())
    }
}

impl<'a> Reply<'a> {
    async fn edit_embed<F>(&mut self, f: F) -> Result<(), serenity::Error>
    where
        F: FnOnce(&mut CreateEmbed) -> &mut CreateEmbed,
    {
        f(&mut self.embed);
        self.message
            .edit(self.ctx, |cr| {
                // poise edit message has different behaviour for embeds than from serenity
                cr.embed(|e| {
                    *e = self.embed.clone();
                    e
                })
            })
            .await
    }
}

impl MessageData {
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
