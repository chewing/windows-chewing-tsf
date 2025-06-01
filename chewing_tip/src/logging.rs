use std::io::Result;

use flexi_logger::{
    Cleanup, Criterion, FileSpec, Logger, LoggerHandle, Naming, WriteMode, writers::LogWriter,
};
use log::Log;

use crate::ts::chewing;

pub(super) struct WinDbgLogWriter;

impl LogWriter for WinDbgLogWriter {
    fn write(&self, _now: &mut flexi_logger::DeferredNow, record: &log::Record) -> Result<()> {
        win_dbg_logger::DEBUGGER_LOGGER.log(record);
        Ok(())
    }
    fn flush(&self) -> Result<()> {
        win_dbg_logger::DEBUGGER_LOGGER.flush();
        Ok(())
    }
}

pub(super) fn init_logger() -> Option<LoggerHandle> {
    let user_dir = chewing::user_dir().ok()?;
    let file_spec = FileSpec::default()
        .directory(user_dir)
        .basename("chewing_tip")
        .suppress_timestamp();
    let writer = Box::new(WinDbgLogWriter);
    match Logger::try_with_env_or_str("warn").and_then(|logger| {
        logger
            .log_to_file_and_writer(file_spec, writer)
            .rotate(
                Criterion::Size(1024 * 1024),
                Naming::Numbers,
                Cleanup::KeepLogFiles(7),
            )
            .append()
            .cleanup_in_background_thread(true)
            .panic_if_error_channel_is_broken(false)
            .use_windows_line_ending()
            .write_mode(WriteMode::BufferAndFlush)
            .start()
    }) {
        Ok(handle) => Some(handle),
        Err(_) => None,
    }
}
