use logforth::{Append, Diagnostic, Error, Layout, layout::PlainTextLayout, record::Record};
use windows::Win32::System::Diagnostics::Debug::{IsDebuggerPresent, OutputDebugStringW};
use windows_core::HSTRING;

#[derive(Debug)]
pub(crate) struct WinDbg {
    layout: Box<dyn Layout>,
}

impl Default for WinDbg {
    fn default() -> Self {
        Self {
            layout: Box::new(PlainTextLayout::default()),
        }
    }
}

impl Append for WinDbg {
    fn append(&self, record: &Record, diags: &[Box<dyn Diagnostic>]) -> Result<(), Error> {
        if is_debugger_present() {
            let mut bytes = self.layout.format(record, diags)?;
            bytes.push(b'\n');
            let text = String::from_utf8_lossy(&bytes);
            output_debug_string(&text);
        }
        Ok(())
    }
    fn flush(&self) -> Result<(), Error> {
        Ok(())
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
