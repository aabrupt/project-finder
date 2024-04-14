use tracing::Level;
use tracing_subscriber::{filter, prelude::*};

pub fn init(level: Option<Level>) {
    let stdout_log = tracing_subscriber::fmt::layer().with_ansi(true).without_time();

    tracing_subscriber::registry()
        .with(stdout_log.with_filter(filter::LevelFilter::from(
            level.unwrap_or(Level::ERROR),
        )))
        .init();
}
