
#[derive(Copy, Clone)]
pub(crate) enum EditCommand<'a> {
    // text to append
    Append(&'a str),
    // old, new
    Substitute(&'a str, &'a str),
    // text to prepend
    Prepend(&'a str),
    // no command
    None,
}

impl<'a> EditCommand<'a> {
    pub fn new(s: &'a str) -> Self {
        let mut edit = EditCommand::None;
        if let Some(command) = s.strip_prefix(".") {
            match command.get(0..1) {
                // append
                Some("a") => {
                    edit = EditCommand::Append(&command[1..]);
                },
                // substitute
                Some("s") => {
                    let text = command[1..].trim_start();
                    let (delim, text) =
                        if let Some(rest) = text.strip_prefix('/') {
                            ('/', rest)
                        } else {
                            (' ', text)
                        };
                    if let Some((old, new)) = text.split_once(delim) {
                        edit = EditCommand::Substitute(old, new);
                    }
                },
                // prepend
                Some("p") => {
                    edit = EditCommand::Prepend(&command[1..]);
                },
                _ => {}
            }
        }
        edit
    }

    pub fn apply(&self, target: &str) -> Option<String> {
        match self {
            EditCommand::Append(x) => Some(format!("{}{}", target, x)),
            EditCommand::Substitute(x, y) => Some(target.replace(x, y)),
            // prepend is a bit different
            //  if it starts with a space, remove and add it at the end
            EditCommand::Prepend(x) => Some({
                if let Some(x) = x.strip_prefix(' ') {
                    format!("{} {}", x, target)
                } else {
                    format!("{}{}", x, target)
                }
            }),
            EditCommand::None => None,
        }
    }
}
