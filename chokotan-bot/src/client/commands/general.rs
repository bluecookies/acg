use poise::serenity_prelude as serenity;
use std::fmt::Write;

use crate::{Data, Error};
type Context<'a> = poise::Context<'a, Data, Error>;
type Command = poise::Command<Data, Error>;

pub(super) fn commands() -> impl IntoIterator<Item = Command> {
    [ping(), help()].into_iter().map(|mut cmd| {
        cmd.category = Some("General");
        cmd
    })
}

/// Ping the bot to see if it is awake
#[poise::command(
    slash_command,
    prefix_command,
    discard_spare_arguments,
    name_localized("fr", "leping")
)]
async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let now: i64 = poise::serenity_prelude::Timestamp::now().timestamp_millis();
    let sent: i64 = ctx.created_at().timestamp_millis();
    let gateway_latency: i64 = ctx.ping().await.as_millis() as i64;
    ctx.reply(format!(
        "pong ({} ms) [gateway: {} ms]",
        now - sent,
        gateway_latency
    ))
    .await?;
    Ok(())
}

/// Fine you can have a help function
#[poise::command(slash_command, prefix_command)]
async fn help(
    ctx: Context<'_>,
    #[description = "Command name"] command: Option<String>,
) -> Result<(), Error> {
    match command {
        Some(command) => help_single_command(ctx, &command).await?,
        None => help_all_commands(ctx).await?,
    }
    Ok(())
}

async fn help_single_command(ctx: Context<'_>, command_name: &str) -> Result<(), serenity::Error> {
    let command = ctx.framework().options().commands.iter().find(|command| {
        if command.name.eq_ignore_ascii_case(command_name) {
            return true;
        }
        if command.aliases.contains(&command_name) {
            return true;
        }
        false
    });

    let reply = if let Some(command) = command {
        match command.help_text {
            Some(f) => f(),
            None => command
                .description
                .as_deref()
                .unwrap_or("No help available")
                .to_owned(),
        }
    } else {
        format!("No such command `{}`", command_name)
    };

    ctx.send(|b| b.content(reply).ephemeral(true)).await?;
    Ok(())
}

async fn help_all_commands(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let mut categories = crate::util::OrderedMap::<&str, Vec<&Command>>::new();
    let mut other = Vec::new();
    let mut max_length = 0;
    for cmd in &ctx.framework().options().commands {
        if let Some(cat) = cmd.category {
            categories.get_or_insert_with(cat, Vec::new).push(cmd);
        } else {
            other.push(cmd);
        }
        max_length = max_length.max(cmd.name.chars().count());
    }
    if !other.is_empty() {
        match categories.get_mut(&"Other") {
            Some(v) => v.extend(other),
            None => categories.insert("Other", other),
        }
    }

    let mut menu = String::from("```\n");
    for (category_name, commands) in categories {
        menu += category_name;
        menu += ":\n";
        for command in commands {
            if command.hide_in_help {
                continue;
            }

            let command_name_length = command.name.chars().count();
            let padding = max_length.saturating_sub(command_name_length) + 1;
            let _ = writeln!(
                menu,
                "  {}{}{}",
                command.name,
                " ".repeat(padding),
                command.description.as_deref().unwrap_or("")
            );
        }
    }

    menu += "\n";
    menu += "\n```";

    ctx.send(|b| b.content(menu).ephemeral(true)).await?;
    Ok(())
}
