use song_artist::GuessSettings;

macro_rules! SETTINGS_FORMAT_STRING {
    () => {
        "\
        **Answer Threshold: ** {:.1}%\n\
        **Display Threshold: ** {:.1}%\n\
        **Space Sensitive: **{}\n\
        **Case Sensitive: **{}\n\
        **Punctuation Sensitive: **{}\n
    "
    };
}

macro_rules! SETTINGS_CHECKMARK {
    ($cond:expr) => {
        if $cond {
            text::CHECKMARK_YES
        } else {
            text::CHECKMARK_NO
        }
    };
}

use crate::{Data, Error};
type Context<'a> = poise::Context<'a, Data, Error>;
type Command = poise::Command<Data, Error>;

use crate::client::messages::text;

pub(super) fn commands() -> impl IntoIterator<Item = Command> {
    [
        set_sensitivity(),
        case_sensitive(),
        space_sensitive(),
        punc_sensitive(),
        show_settings(),
    ]
}

#[derive(poise::ChoiceParameter)]
enum Strictness {
    Case,
    Space,
    Punctuation,
}

/// Set strictness for case, space and punctuation
#[poise::command(slash_command, prefix_command, rename = "strict")]
async fn set_sensitivity(
    ctx: Context<'_>,
    #[description = "Strictness type to set"] kind: Option<Strictness>,
    #[rename = "value"] status: Option<bool>,
) -> Result<(), Error> {
    if let Some(kind) = kind {
        set_quiz_sensitive(ctx, kind, status).await
    } else {
        send_settings_embed(ctx, Some("`kind` can be `Case`, `Space` or `Punctuation`")).await?;
        Ok(())
    }
}

/// Set case sensitivity for song artist
#[poise::command(slash_command, prefix_command, hide_in_help)]
async fn case_sensitive(ctx: Context<'_>, status: Option<bool>) -> Result<(), Error> {
    set_quiz_sensitive(ctx, Strictness::Case, status).await
}

/// Set space sensitivity for song artist
#[poise::command(slash_command, prefix_command, hide_in_help)]
async fn space_sensitive(ctx: Context<'_>, status: Option<bool>) -> Result<(), Error> {
    set_quiz_sensitive(ctx, Strictness::Space, status).await
}

/// Set punctuation sensitivity for song artist
#[poise::command(slash_command, prefix_command, hide_in_help)]
async fn punc_sensitive(ctx: Context<'_>, status: Option<bool>) -> Result<(), Error> {
    set_quiz_sensitive(ctx, Strictness::Punctuation, status).await
}

/// Show the current strictness settings for song artist
#[poise::command(rename = "settings", slash_command, prefix_command)]
async fn show_settings(ctx: Context<'_>) -> Result<(), Error> {
    send_settings_embed(ctx, None).await
}

async fn set_quiz_sensitive(
    ctx: Context<'_>,
    kind: Strictness,
    status: Option<bool>,
) -> Result<(), Error> {
    let quiz = &ctx.data().quiz;

    let status = match kind {
        Strictness::Case => quiz.set_case_sensitive(status),
        Strictness::Space => quiz.set_space_sensitive(status),
        Strictness::Punctuation => quiz.set_punctuation_sensitive(status),
    };

    let setting = match kind {
        Strictness::Case => "Case sensitivity",
        Strictness::Space => "Space sensitivity",
        Strictness::Punctuation => "Punctuation sensitivity",
    };

    ctx.reply(format!(
        "{} is now {}.",
        setting,
        if status { "ON" } else { "OFF" }
    ))
    .await?;

    Ok(())
}

async fn send_settings_embed(ctx: Context<'_>, content: Option<&str>) -> Result<(), Error> {
    let quiz = &ctx.data().quiz;
    let settings = quiz.settings();

    ctx.send(|cm| {
        if let Some(c) = content {
            cm.content(c);
        }
        cm.embed(|ce| ce.description(create_settings_desc(&settings)))
            .reply(true)
    })
    .await?;
    Ok(())
}

fn create_settings_desc(settings: &GuessSettings) -> String {
    format!(
        SETTINGS_FORMAT_STRING!(),
        &settings.answer_threshold * 100.0,
        &settings.display_threshold * 100.0,
        SETTINGS_CHECKMARK!(settings.space_sensitive),
        SETTINGS_CHECKMARK!(settings.case_sensitive),
        SETTINGS_CHECKMARK!(settings.punc_sensitive),
    )
}
