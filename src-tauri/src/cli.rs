//! Commands for a running LyricHub from the command line (IDEEN.md, package D), for Stream Deck,
//! AutoHotkey and the like: `lyrichub.exe --toggle`, `--edit`, `--layout`, `--romaji` and
//! `--nudge <ms>`. A second start hands its arguments to the running instance (single instance
//! plugin); a first start with a command starts minimised.

use tauri::AppHandle;

use crate::hotkeys::Action;

/// One nudge from the command line moves the song offset at most this far.
const NUDGE_MAX_MS: i32 = 2000;

#[derive(Debug, PartialEq)]
pub enum Command {
    Action(Action),
    Nudge(i32),
}

/// The commands among the arguments (the program path and unknown ones are skipped).
pub fn parse(args: &[String]) -> Vec<Command> {
    let mut out = Vec::new();
    let mut it = args.iter().peekable();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--toggle" => out.push(Command::Action(Action::Toggle)),
            "--edit" => out.push(Command::Action(Action::Edit)),
            "--layout" => out.push(Command::Action(Action::Layout)),
            "--romaji" => out.push(Command::Action(Action::Romaji)),
            "--nudge" => {
                if let Some(ms) = it.peek().and_then(|v| v.parse::<i32>().ok()) {
                    it.next();
                    out.push(Command::Nudge(ms.clamp(-NUDGE_MAX_MS, NUDGE_MAX_MS)));
                }
            }
            _ => {}
        }
    }
    out
}

pub fn run(app: &AppHandle, commands: &[Command]) {
    for c in commands {
        tracing::info!(command = ?c, "command from the command line");
        match c {
            Command::Action(a) => crate::hotkeys::perform(app, *a),
            Command::Nudge(ms) => {
                crate::ipc::nudge(app, *ms);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn commands_are_read_from_the_arguments() {
        assert_eq!(parse(&args(r"C:\x\lyrichub.exe --toggle")), [Command::Action(Action::Toggle)]);
        assert_eq!(parse(&args("lyrichub.exe --nudge -100 --layout")), [Command::Nudge(-100), Command::Action(Action::Layout)]);
        assert_eq!(parse(&args("lyrichub.exe --nudge 9000")), [Command::Nudge(NUDGE_MAX_MS)]);
        assert_eq!(parse(&args("lyrichub.exe --nudge --romaji")), [Command::Action(Action::Romaji)], "a nudge without a number");
        assert!(parse(&args("lyrichub.exe --minimized --unknown")).is_empty());
    }
}
