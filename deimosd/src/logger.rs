use log::Level;


/// Barebones logger that writes all log messages to stderr
pub struct Logger;

impl Logger {
    const INSTANCE: Self = Self;

    pub fn install() -> Result<(), log::SetLoggerError> {
        log::set_logger(&Self::INSTANCE)?;
        log::set_max_level(log::LevelFilter::Trace);
        Ok(())
    }
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        eprintln!(
            "[{}] {}",
            match record.level() {
                Level::Trace => "TRA",
                Level::Debug => "DBG",
                Level::Info  => "INF",
                Level::Warn  => "WRN",
                Level::Error => "ERR"
            },
            record.args()
        )
    }

    fn flush(&self) {
        
    }
}
