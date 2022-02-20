use std::io::{Stdout, Write};
use termion::raw::RawTerminal;

use crate::audio::Volume;
use crate::events::UiMode;
use crate::state::AppState;

pub fn draw(out: &mut RawTerminal<Stdout>, state: &AppState) {
    let start = termion::cursor::Goto(1, 2);
    let clear_line = termion::clear::CurrentLine;
    let title = match state.mode {
        UiMode::View => "Audio Devices",
        UiMode::EditInput => "Update Input",
        UiMode::EditOutput => "Update Output",
    };
    let list = draw_list(state);
    let mods = &state.key_modifiers;
    let keys = &state.keys;
    write!(
        out,
        "{start}{clear_line}{title}\r
-------------\r
{list}\r-------------\r
{clear_line}Keys: {mods:?}{keys:?}\r
"
    )
    .unwrap();
    out.flush().unwrap();
}

fn draw_list(state: &AppState) -> String {
    let mut list = String::new();
    let longest_name_len = state
        .audio
        .device_list()
        .iter()
        .fold(0, |acc, (_, _, _, device)| {
            if device.name.len() > acc {
                device.name.len()
            } else {
                acc
            }
        });
    for (active_in, active_out, muted, device) in state.audio.device_list() {
        let mark = match (active_in, active_out) {
            (true, true) => "<->",
            (true, false) => "-->",
            (false, true) => "<--",
            (false, false) => "   ",
        };
        let levels_in = {
            if let Some((vol, mute)) = state.audio.input(&device.id) {
                draw_level(Some(vol), mute)
            } else {
                draw_level(None, false)
            }
        };
        let levels_out = {
            if let Some((vol, mute)) = state.audio.output(&device.id) {
                draw_level(Some(vol), mute)
            } else {
                draw_level(None, false)
            }
        };
        let spaces = " ".repeat(longest_name_len - device.name.len());
        let item = format!(
            "{}{} {}{} : {} | {}\r\n",
            termion::clear::CurrentLine,
            mark,
            device.name,
            spaces,
            levels_in,
            levels_out
        );
        list.push_str(&item);
    }
    list
}

fn draw_level(volume: Option<f32>, muted: bool) -> String {
    match volume {
        Some(vol) => {
            if vol == 0.0 || muted {
                return "░".repeat(10);
            }
            let steps = (vol * 10.0) as usize;
            let amount = "▓".repeat(steps);
            let fill = "▒".repeat(10 - steps);
            format!("{}{}", amount, fill)
        }
        None => "·".repeat(10),
    }
}
