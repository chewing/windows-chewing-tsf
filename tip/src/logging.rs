// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use interprocess::os::windows::ToWtf16;
use logforth::{Append, Diagnostic, Error, Layout, layout::PlainTextLayout, record::Record};
use windows::Win32::System::Diagnostics::Debug::{IsDebuggerPresent, OutputDebugStringW};
use windows_core::HSTRING;

#[derive(Debug)]
pub(crate) struct WinDbg {
    layout: Box<dyn Layout>,
    socket: UdpSocket,
}

impl Default for WinDbg {
    fn default() -> Self {
        Self {
            layout: Box::new(PlainTextLayout::default()),
            socket: UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap(),
        }
    }
}

impl Append for WinDbg {
    fn append(&self, record: &Record, diags: &[Box<dyn Diagnostic>]) -> Result<(), Error> {
        if is_debugger_present() {
            let mut bytes = self.layout.format(record, diags)?;
            bytes.truncate(1999);
            bytes.push(b'\n');
            let text = String::from_utf8_lossy(&bytes);
            output_debug_string(&text);
            if let Ok(ucs2text) = text.to_wtf_16() {
                self.socket
                    .send_to(
                        &ucs2text.to_os_string().as_encoded_bytes(),
                        SocketAddr::from((Ipv4Addr::new(127, 0, 0, 1), 2020)),
                    )
                    .map_err(Error::from_io_error)?;
            }
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

pub(crate) fn is_debugger_present() -> bool {
    unsafe { IsDebuggerPresent().as_bool() }
}
