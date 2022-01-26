use crate::audio::AudioState;
use crate::events::UiMode;

#[derive(Debug)]
pub struct AppState {
    pub audio: AudioState,
    pub keys: Vec<i64>,
    pub key_modifiers: Vec<String>,
    pub mode: UiMode,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            audio: AudioState::new(),
            keys: Vec::new(),
            key_modifiers: Vec::new(),
            mode: UiMode::View,
        }
    }
}
