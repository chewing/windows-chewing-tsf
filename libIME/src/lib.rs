mod gfx;
mod window;

#[no_mangle]
unsafe extern "C" fn LibIME2Init() {
    win_dbg_logger::rust_win_dbg_logger_init_debug();
}
