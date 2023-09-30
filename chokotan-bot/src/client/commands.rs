mod general;
mod quiz;
mod songs;

use crate::{Data, Error};
type Command = poise::Command<Data, Error>;

pub(crate) fn commands() -> Vec<Command> {
    let it = std::iter::empty()
        .chain(general::commands())
        .chain(songs::commands())
        .chain(quiz::commands())
        .map(|mut c| {
            c.name = c.name.replace("_", "");
            c
        });
    it.collect()
}
