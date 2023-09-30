use poise::serenity_prelude as serenity;
use serenity::{ChannelId, Http, Message};
use serenity::{CreateEmbed, CreateMessage};

use crate::quiz::QuizSongData;
use crate::Error;

use super::text::*;
use super::{colours::*, format_time, value_to_string};

// We don't use the poise Context because quizzes are so long running
//  that we want to be able to spawn the task to run the quiz
//  and complete the quiz start command
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
        let message = channel_id
            .send_message(&http, quiz_song_message(song_num, loaded))
            .await?;
        Ok(QuizSongMessage { http, message })
    }

    pub(crate) async fn set_loaded(&mut self) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            embed.colour(LOADED_COLOUR).description(LOADED_SONG_TEXT);
            self.message
                .edit(self.http.as_ref(), |em| em.set_embed(embed))
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn set_playing(&mut self) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            embed.colour(PLAYING_COLOUR).description(PLAYING_TEXT);
            self.message
                .edit(self.http.as_ref(), |em| em.set_embed(embed))
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn set_error(
        &mut self,
        e: Error,
        song_data: QuizSongData,
    ) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            embed
                .colour(ERROR_COLOUR)
                .description(format!("**Error: **{}\n\n", e));

            set_song_data(&mut embed, song_data);

            self.message
                .edit(self.http.as_ref(), |em| em.set_embed(embed))
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn set_finished(mut self, song_data: QuizSongData) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            embed.colour(FINISHED_COLOUR).description("");
            set_song_data(&mut embed, song_data);
            self.message
                .channel_id
                .send_message(self.http.as_ref(), |cm| {
                    cm.embed(|em| {
                        *em = embed;
                        em
                    })
                })
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn set_cancelled(&mut self, song_data: QuizSongData) -> Result<(), Error> {
        if let Some(mut embed) = self.pop_embed() {
            embed.colour(ERROR_COLOUR).description(CANCELLED_SONG_TEXT);
            set_song_data(&mut embed, song_data);
            self.message
                .edit(self.http.as_ref(), |em| em.set_embed(embed))
                .await?;
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
) -> impl for<'a> FnOnce(&'a mut CreateMessage<'b>) -> &'a mut CreateMessage<'b> + Send {
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

fn set_song_data(embed: &mut CreateEmbed, song_data: QuizSongData) {
    if let Some(info) = song_data.song_info {
        for crate::quiz::QuizInfoField { name, col, .. } in song_data.display_fields.iter() {
            match col.as_ref() {
                "songname" => {
                    embed.field(name, &info.song_name, false);
                }
                "artist" => {
                    embed.field(name, &info.artist, false);
                }
                c => {
                    if let Some(value) = info.fields.get(c) {
                        embed.field(name, value_to_string(value), false);
                    }
                }
            }
        }
    }

    let sample: std::borrow::Cow<_> = if let Some(t) = song_data.sample {
        format_time(t).into()
    } else {
        "?:??".into()
    };
    let duration: std::borrow::Cow<_> = if let Some(t) = song_data.duration {
        format_time(t).into()
    } else {
        "?:??".into()
    };
    embed.field("Sample", format!("{}/{}", sample, duration), false);
}
