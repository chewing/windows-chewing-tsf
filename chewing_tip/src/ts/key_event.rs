pub(super) enum KeyEventKind {
    KeyDown,
    KeyUp,
}

pub(super) struct KeyEvent {
    pub(super) kind: KeyEventKind,
    pub(super) vk: usize,
    pub(super) lparam: isize,
}

impl KeyEvent {
    pub(super) fn down(vk: usize, lparam: isize) -> KeyEvent {
        Self::new(KeyEventKind::KeyDown, vk, lparam)
    }
    pub(super) fn up(vk: usize, lparam: isize) -> KeyEvent {
        Self::new(KeyEventKind::KeyUp, vk, lparam)
    }
    fn new(kind: KeyEventKind, vk: usize, lparam: isize) -> KeyEvent {
        KeyEvent { kind, vk, lparam }
    }
}
