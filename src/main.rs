use std::io::{stdin, stdout, Write};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

mod audio;
mod coreaudio;
mod events;
mod state;
mod tui;

use crate::audio::Channel;
use crate::events::{Action, UiMode};
use crate::state::AppState;
use crate::tui::draw;

fn main() {
    let stdout = stdout();
    let mut stdout = stdout.into_raw_mode().unwrap();
    let stdin = stdin();
    let mut state = AppState::new();
    let has_full_access = events::request_accessibility_access();
    if !has_full_access {
        panic!("Need accessibility and input permissions.");
    }
    // println!("Test: {has_full_access}!");

    // Listen for events in separate threads
    let (tx1, rx) = channel();
    let tx2 = tx1.clone();
    let tx3 = tx1.clone();
    thread::spawn(move || {
        // Tap into OS key events (no focus required)
        events::event_tap(|action| tx1.send(action).unwrap()).unwrap();
    });
    thread::spawn(move || {
        // Terminal key events for focused control
        for c in stdin.keys() {
            match c.unwrap() {
                Key::Ctrl('c') => tx2.send(Action::Exit).unwrap(),
                Key::Char('i') => tx2.send(Action::ModeSwitch(UiMode::EditInput)).unwrap(),
                Key::Char('o') => tx2.send(Action::ModeSwitch(UiMode::EditOutput)).unwrap(),
                Key::Esc => tx2.send(Action::ModeSwitch(UiMode::View)).unwrap(),
                Key::Up => tx2.send(Action::SelectPrev).unwrap(),
                Key::Down => tx2.send(Action::SelectNext).unwrap(),
                Key::Left => tx2.send(Action::VolumeDown).unwrap(),
                Key::Right => tx2.send(Action::VolumeUp).unwrap(),
                Key::Char('/') => tx2.send(Action::ToggleMute).unwrap(),
                _ => {}
            }
        }
    });
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(500));
        tx3.send(Action::Poll).unwrap();
    });

    // Initial draw
    println!("{}{}", termion::clear::All, termion::cursor::Hide);
    draw(&mut stdout, &state);

    loop {
        // Waiting for events
        match rx.recv().unwrap() {
            Action::KeyDown {
                key_code,
                modifiers,
                repeating,
            } => {
                if !repeating {
                    state.keys.push(key_code);
                    state.key_modifiers = modifiers.list_active();
                    draw(&mut stdout, &state);
                }
            }
            Action::KeyUp {
                key_code,
                modifiers,
            } => {
                if let Some(i) = state.keys.iter().position(|k| *k == key_code) {
                    state.keys.remove(i);
                    state.key_modifiers = modifiers.list_active();
                    draw(&mut stdout, &state);
                }
            }
            Action::Modifier { modifiers } => {
                state.key_modifiers = modifiers.list_active();
                draw(&mut stdout, &state);
            }
            Action::ModeSwitch(mode) => {
                state.mode = mode;
                draw(&mut stdout, &state);
            }
            Action::SelectNext => {
                match state.mode {
                    UiMode::EditInput => {
                        state.audio.next_input();
                    }
                    UiMode::EditOutput => {
                        state.audio.next_output();
                    }
                    _ => continue,
                };
                draw(&mut stdout, &state);
            }
            Action::SelectPrev => {
                match state.mode {
                    UiMode::EditInput => {
                        state.audio.prev_input();
                    }
                    UiMode::EditOutput => {
                        state.audio.prev_output();
                    }
                    _ => continue,
                };
                draw(&mut stdout, &state);
            }
            Action::ToggleMute => {
                match state.mode {
                    UiMode::EditInput => {
                        state.audio.toggle_mute(Channel::Input);
                    }
                    UiMode::EditOutput => {
                        state.audio.toggle_mute(Channel::Output);
                    }
                    _ => continue,
                };
                draw(&mut stdout, &state);
            }
            Action::VolumeUp => {
                match state.mode {
                    UiMode::EditInput => {
                        state.audio.move_volume(Channel::Input, 0.1);
                    }
                    UiMode::EditOutput => {
                        state.audio.move_volume(Channel::Output, 0.1);
                    }
                    _ => continue,
                };
                draw(&mut stdout, &state);
            }
            Action::VolumeDown => {
                match state.mode {
                    UiMode::EditInput => {
                        state.audio.move_volume(Channel::Input, -0.1);
                    }
                    UiMode::EditOutput => {
                        state.audio.move_volume(Channel::Output, -0.1);
                    }
                    _ => continue,
                };
                draw(&mut stdout, &state);
            }
            Action::Poll => {
                state.audio.update();
                draw(&mut stdout, &state);
            }
            Action::Exit => break,
        }
    }

    // Clean up before exit
    write!(&mut stdout, "{}", termion::cursor::Show).unwrap();
    stdout.flush().unwrap();
}
