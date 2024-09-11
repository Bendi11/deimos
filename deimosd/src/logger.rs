use log::Level;
use tracing::span;


/// Barebones logger that writes all log messages to stderr
pub struct Logger;

impl Logger {
    const INSTANCE: Self = Self;

    pub fn install() -> Result<(), log::SetLoggerError> {
        log::set_logger(&Self::INSTANCE)?;
        log::set_max_level(log::LevelFilter::Trace);
        tracing::subscriber::set_global_default(Self::INSTANCE).unwrap();
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

impl tracing::Subscriber for Logger {
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) {
        
    }

    fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
        
    }

    fn enter(&self, span: &span::Id) {
        
    }

    fn exit(&self, span: &span::Id) {
        
    }

    fn event(&self, event: &tracing::Event<'_>) {
        eprintln!(
            "{:?}",
            event
        )
    }
}
