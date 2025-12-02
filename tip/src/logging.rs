use std::io::Write;

use windows::Win32::System::Diagnostics::Debug::{IsDebuggerPresent, OutputDebugStringW};
use windows_core::HSTRING;

#[derive(Default)]
pub(crate) struct WinDbgWriter {
    buffer: Vec<u8>,
}

impl Write for WinDbgWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if is_debugger_present() {
            self.buffer.write(buf)
        } else {
            Ok(buf.len())
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if is_debugger_present() {
            let text = String::from_utf8_lossy(&self.buffer);
            output_debug_string(&text);
        }
        Ok(())
    }
}

impl Drop for WinDbgWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

pub(crate) fn output_debug_string(text: &str) {
    unsafe {
        OutputDebugStringW(&HSTRING::from(text));
    }
}

fn is_debugger_present() -> bool {
    unsafe { IsDebuggerPresent().as_bool() }
}
