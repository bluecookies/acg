use std::collections::HashSet;
use std::sync::Arc;
use itertools::Itertools;
use crate::bot::Bot;
use crate::Error;

pub struct Command {
    name: String,
    kind: String,
    hidden: bool,
    description: Vec<String>,
    // args, sender, bot
    pub(crate) handler: Arc<dyn (Fn(&mut dyn Iterator<Item=&str>, &str, &Bot) -> Result<(), Error>) + Send + Sync + 'static>,
}


// TODO:
//   spec, leave, lobby
impl Bot {
    pub fn register_commands(&self) {
        // Help commands
        self.register_command("help", "help", |args, _sender, bot| {
            if let Some(command) = args.next() {
                let guard = bot.commands.lock().expect("mutex poisoned");
                if let Some(cmd) = guard.get(command) {
                    for msg in cmd.description.iter() {
                        bot.client.send_chat_message(msg);
                    }
                } else {
                    bot.client.send_chat_message(format!("Command {} not found", command));
                }
            } else {
                for msg in &["Usage:", "/help <command>", "Shows help on a command", "/commands <type>", "Lists commands"] {
                    bot.client.send_chat_message(msg);
                }
            }
            Ok(())
        }, &["/help", "/help <command>"], true);

        self.register_command("commands", "help", |args, _sender, bot| {
            let guard = bot.commands.lock().expect("mutex poisoned");
            if let Some(kind) = args.next() {
                // list the commands
                let mut cmds = HashSet::new();
                for cmd in guard.values() {
                    if !cmd.hidden && (cmd.kind == kind || kind == "all") {
                        cmds.insert(&cmd.name);
                    }
                }
                if cmds.is_empty() {
                    bot.client.send_chat_message(format!("No commands found of type {}", kind));
                } else {
                    bot.client.send_chat_message(format!("Commands: {}", cmds.iter().format(", ")));
                };
            } else {
                // list the command types
                let mut kinds = HashSet::new();
                for cmd in guard.values() {
                    if !cmd.hidden {
                        kinds.insert(&cmd.kind);
                    }
                }
                if kinds.is_empty() {
                    bot.client.send_chat_message("No command types found");
                } else {
                    bot.client.send_chat_message(format!("Command types: {}", kinds.iter().format(", ")));
                };
            }
            Ok(())
        }, &["/commands", "/commands <type>", "Lists the command types/commands"], true);

        // Lobby commands
        self.register_command("join", "lobby", |_args, _sender, bot| {
            bot.client.change_to_player()?;
            bot.client.set_ready(true)?;  // TODO: set ready when own state changed
            Ok(())
        }, &["/join", "Joins the lobby"], true);

        self.register_command("host", "lobby", |args, sender, bot| {
            let host = args.next().unwrap_or(sender);
            bot.client.promote_host(host)?;
            Ok(())
        }, &["/host", "/host <target>", "Promotes host"], true);

        self.register_command("start", "lobby", |_args, _sender, bot| {
            bot.client.start_game()?;
            Ok(())
        }, &["/start", "Starts game"], true);

        // Game
        self.register_command("skip", "game", |_args, _sender, bot| {
            bot.client.vote_skip()?;
            Ok(())
        }, &["/skip", "Votes skip"], true);

        // settings
        self.register_command("set", "settings", |args, sender, bot| {
            match args.next() {
                Some("casesensitive") => {
                    let sensitive = bot.quiz.toggle_case_sensitive();
                    let msg = format!("@{} Case sensitivity is now {}", sender, if sensitive { "ON" } else { "OFF" });
                    bot.client.send_chat_message(msg);
                },
                Some("spacesensitive") => {
                    let sensitive = bot.quiz.toggle_space_sensitive();
                    let msg = format!("@{} Space sensitivity is now {}", sender, if sensitive { "ON" } else { "OFF" });
                    bot.client.send_chat_message(msg);
                },
                Some("puncsensitive") => {
                    let sensitive = bot.quiz.toggle_punctuation_sensitive();
                    let msg = format!("@{} Punctuation sensitivity is now {}", sender, if sensitive { "ON" } else { "OFF" });
                    bot.client.send_chat_message(msg);
                },
                _ => {
                    let msg = format!("@{} settings: casesensitive, spacesensitive, puncsensitive", sender);
                    bot.client.send_chat_message(msg);
                },
            }
            Ok(())
        }, &["/set <settings>", "Sets S/A quiz settings"], true);

        self.register_command("setlist", "settings", |args, sender, bot| {
            match args.next().map(|s| s.to_uppercase()).as_deref() {
                Some(list_type @ ("MAL" | "ANILIST" | "KITSU")) => {
                    let list_type = amq_bot::ListType::try_from(list_type).expect("validated list types failed");
                    if !bot.client.set_list(list_type, args.next(), Some(sender.to_string()))? {
                        bot.client.send_chat_message("already setting list");
                    }
                },
                _ => {
                    let msg = format!("@{} list type must be mal, anilist or kitsu", sender);
                    bot.client.send_chat_message(msg);
                }
            }
            Ok(())
        }, &["/setlist <listtype> [listname]", "Sets/removes anime list"], true);


        // "dbg_data" => {
        //     let data = self.client.game_data();
        //     self.client.send_chat_message(format!("{:?}", data));
        // },
    }

    fn register_command(
        &self,
        cmd: &str,
        kind: &str,
        cb: impl (Fn(&mut dyn Iterator<Item=&str>, &str, &Bot) -> Result<(), Error>) + Send + Sync + 'static,
        usage: &[&str],
        visible: bool
    ) {
        let mut commands = self.commands.lock().expect("mutex poisoned");
        let cmd_name = cmd.to_string();
        let cmd = Command {
            name: cmd_name.clone(),
            kind: kind.to_string(),
            hidden: !visible,
            description: usage.into_iter().map(|s| s.to_string()).collect(),
            handler: Arc::new(cb),
        };
        commands.insert(cmd_name, cmd);
    }
}
