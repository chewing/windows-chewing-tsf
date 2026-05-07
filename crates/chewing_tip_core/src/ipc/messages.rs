use serde::{Deserialize, Serialize};

// TODO: make sure the coordinate is DPI aware
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ShowNotification {
    pub position: Position,
    pub text: String,
    pub font_family: String,
    pub font_size: f32,
    pub fg_color: String,
    pub bg_color: String,
    pub border_color: String,
}
pub type ShowNotificationReply = ();
impl ShowNotification {
    pub const METHOD: &str = "im.chewing.ui.ShowNotification";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ShowCandidateList {
    pub position: Position,
    pub items: Vec<String>,
    pub selkeys: Vec<u16>,
    pub total_page: u32,
    pub current_page: u32,
    pub font_family: String,
    pub font_size: f32,
    pub cand_per_row: u32,
    pub use_cursor: bool,
    pub current_sel: usize,
    pub selkey_color: String,
    pub fg_color: String,
    pub bg_color: String,
    pub highlight_fg_color: String,
    pub highlight_bg_color: String,
    pub border_color: String,
}
pub type ShowCandidateListReply = ();
impl ShowCandidateList {
    pub const METHOD: &str = "im.chewing.ui.ShowCandidateList";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct HideCandidateList;
pub type HideCandidateListReply = ();
impl HideCandidateList {
    pub const METHOD: &str = "im.chewing.ui.HideCandidateList";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Stop;
pub type StopReply = ();
impl Stop {
    pub const METHOD: &str = "im.chewing.ui.Stop";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct CheckUpdate;
pub type CheckUpdateReply = ();
impl CheckUpdate {
    pub const METHOD: &str = "im.chewing.ui.CheckUpdate";
}
