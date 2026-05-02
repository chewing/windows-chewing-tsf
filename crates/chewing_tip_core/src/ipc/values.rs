use base64_serde::base64_serde_type;
use serde::{Deserialize, Serialize};

base64_serde_type!(Base64Standard, base64::engine::general_purpose::STANDARD);

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub enum IpcShiftKeyState {
    Down,
    Consumed,
    #[default]
    Up,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct IpcKeyEvent {
    pub vk: u16,
    pub scan_code: u16,
    pub ascii_code: u8,
    #[serde(with = "Base64Standard")]
    pub key_state: Vec<u8>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Composition {
    pub commit: String,
    pub preedit: String,
    pub segments: Vec<(usize, usize)>,
    pub cursor: usize,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct CandidateList {
    pub items: Vec<String>,
    pub selkeys: Vec<char>,
    pub total_page: u32,
    pub current_page: u32,
}
