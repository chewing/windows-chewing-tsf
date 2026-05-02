use serde::{Deserialize, Serialize};

use crate::ipc::values::{CandidateList, Composition, IpcKeyEvent, IpcShiftKeyState};

use super::values::Position;

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

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OnTestKeyDown {
    pub is_context_mutable: bool,
    pub is_composing: bool,
    pub shift_key_state: IpcShiftKeyState,
    pub event: IpcKeyEvent,
}
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OnTestKeyDownReply {
    pub handled: bool,
}
impl OnTestKeyDown {
    pub const METHOD: &str = "im.chewing.tip.OnTestKeyDown";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OnKeyDown {
    pub event: IpcKeyEvent,
}
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OnKeyDownReply {
    pub handled: bool,
    pub composition: Option<Composition>,
    pub candidate_list: Option<CandidateList>,
    pub notification: Option<String>,
}
impl OnKeyDown {
    pub const METHOD: &str = "im.chewing.tip.OnKeydown";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OnTestKeyUp {
    pub event: IpcKeyEvent,
}
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OnTestKeyUpReply {
    pub handled: bool,
}
impl OnTestKeyUp {
    pub const METHOD: &str = "im.chewing.tip.OnTestKeyUp";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OnKeyUp {
    pub event: IpcKeyEvent,
}
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OnKeyUpReply {
    pub handled: bool,
    pub composition: Option<Composition>,
    pub candidate_list: Option<CandidateList>,
    pub notification: Option<String>,
}
impl OnKeyUp {
    pub const METHOD: &str = "im.chewing.tip.OnKeyUp";
}
