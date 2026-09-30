use core::arch::asm;

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

// ========= Floating Point Environment Normalization

#[inline]
unsafe fn stmxcsr() -> u32 {
    let mut mx: u32 = 0;
    unsafe {
        asm!(
            "stmxcsr dword ptr [{p}]",
            p = in(reg) &mut mx,
        );
    }
    mx
}

#[inline]
unsafe fn ldmxcsr(mx: u32) {
    unsafe {
        asm!(
            "ldmxcsr dword ptr [{p}]",
            p = in(reg) &mx,
        );
    }
}

#[cfg(target_arch = "x86")]
#[inline]
unsafe fn fnstcw() -> u16 {
    let mut cw: u16 = 0;
    unsafe {
        asm!(
            "fnstcw word ptr [{p}]",
            p = in(reg) &mut cw,
        );
    }
    cw
}

#[cfg(target_arch = "x86")]
#[inline]
unsafe fn fldcw(cw: u16) {
    unsafe {
        asm!(
            "fldcw word ptr [{p}]",
            p = in(reg) &cw,
        );
    }
}

const MXCSR_RUST_DEFAULT: u32 = 0x1F80;

#[cfg(target_arch = "x86_64")]
pub struct FpGuard {
    saved_mxcsr: u32,
}

#[cfg(target_arch = "x86_64")]
impl FpGuard {
    /// Enter a "rust-standard" FP environment, restoring the
    /// host's (Delphi's) environment on drop.
    #[inline]
    pub fn enter() -> Self {
        let saved_mxcsr = unsafe { stmxcsr() };
        unsafe { ldmxcsr(MXCSR_RUST_DEFAULT) };
        FpGuard { saved_mxcsr }
    }
}

#[cfg(target_arch = "x86_64")]
impl Drop for FpGuard {
    #[inline]
    fn drop(&mut self) {
        unsafe { ldmxcsr(self.saved_mxcsr) }
    }
}

#[cfg(target_arch = "x86")]
pub struct FpGuard {
    saved_cw: u16,
    saved_mxcsr: u32,
}

#[cfg(target_arch = "x86")]
impl FpGuard {
    #[inline]
    pub fn enter() -> Self {
        let saved_cw = unsafe { fnstcw() };
        // 0x027F: extended precision, round-to-nearest, all exceptions masked
        unsafe { fldcw(0x027F) };

        let saved_mxcsr = unsafe { stmxcsr() };
        unsafe { ldmxcsr(MXCSR_RUST_DEFAULT) };
        FpGuard {
            saved_cw,
            saved_mxcsr,
        }
    }
}

#[cfg(target_arch = "x86")]
impl Drop for FpGuard {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            fldcw(self.saved_cw);
            ldmxcsr(self.saved_mxcsr);
        }
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
pub(crate) struct FpGuard;

#[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
impl FpGuard {
    pub fn enter() -> Self {
        FpGuard
    }
}
