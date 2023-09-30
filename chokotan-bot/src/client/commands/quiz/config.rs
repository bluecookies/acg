use crate::{Data, Error};
type Context<'a> = poise::Context<'a, Data, Error>;
type Command = poise::Command<Data, Error>;

pub(super) fn commands() -> impl IntoIterator<Item = Command> {
    [reload_config(), quiz_types()]
}

// TODO: owner only
/// Reload quiz configurations from file
#[poise::command(slash_command, prefix_command)]
async fn reload_config(ctx: Context<'_>) -> Result<(), Error> {
    let quiz = &ctx.data().quiz;
    let loaded_configs = quiz.reload_config()?;
    ctx.reply(format!("Read {} quiz configs", loaded_configs.num_configs))
        .await?;
    if loaded_configs.num_dupes != 0 {
        ctx.reply(format!(
            "{} duplicate configurations found.",
            loaded_configs.num_dupes
        ))
        .await?;
    }
    for (error, path) in loaded_configs.errors {
        ctx.reply(format!("Error reading config for `{}`: {}", path, error))
            .await?;
    }
    Ok(())
}

// TODO: make sure this is not beyond the message size limit
/// List quiz configurations
#[poise::command(slash_command, prefix_command)]
async fn quiz_types(ctx: Context<'_>) -> Result<(), Error> {
    let configs = ctx.data().quiz.configs();
    let mut list = String::from("```\n");
    let mut cfg_list = Vec::with_capacity(configs.len());
    for cfg in configs.values() {
        cfg_list.push((cfg.name(), cfg.description()));
    }
    // Calculate the maximum length for padding
    let max_length =
        if let Some(name) = cfg_list.iter().max_by_key(|(name, _)| name.chars().count()) {
            name.0.chars().count()
        } else {
            ctx.reply("No quiz configurations loaded!").await?;
            return Ok(());
        };

    for (name, desc) in cfg_list {
        let padding = max_length.saturating_sub(name.chars().count()) + 1;
        list += name;
        list += &" ".repeat(padding);
        list += "- ";
        if let Some(desc) = desc {
            list += desc;
        }
        list += "\n";
    }
    list += "\n```";
    ctx.reply(list).await?;
    Ok(())
}
