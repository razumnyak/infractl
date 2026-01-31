use crate::cli::Cli;
use anyhow::Result;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

pub fn init(cli: &Cli) -> Result<()> {
    let log_level = cli.effective_log_level();
    let log_format = cli.effective_log_format();

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    let subscriber = tracing_subscriber::registry().with(env_filter);

    match log_format {
        "json" => {
            let fmt_layer = fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false)
                .with_span_events(FmtSpan::CLOSE);

            subscriber.with(fmt_layer).init();
        }
        _ => {
            let fmt_layer = fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false)
                .with_span_events(FmtSpan::CLOSE);

            subscriber.with(fmt_layer).init();
        }
    }

    Ok(())
}
