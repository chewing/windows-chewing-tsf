use windows::Win32::{Foundation::MAX_PATH, System::LibraryLoader::GetModuleFileNameW};

pub(crate) struct Quirk {
    /// Application is not compatiable with our IMM32 patching
    pub skip_imm32_patch: bool,
}

impl Quirk {
    pub(crate) fn query() -> Option<Quirk> {
        let mut buffer = [0u16; MAX_PATH as usize];
        let len = unsafe { GetModuleFileNameW(None, &mut buffer[..]) as usize };
        if len == 0 {
            return None;
        }
        buffer[len - 1] = 0;
        let exe_path = String::from_utf16_lossy(&buffer[..(len as usize)]);
        if exe_path.ends_with(r"\MyAB.exe") {
            return Some(Quirk {
                skip_imm32_patch: true,
            });
        }
        None
    }
}
