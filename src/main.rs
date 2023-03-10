pub mod command_handler;
pub mod conversation;
pub mod helper;
pub mod msg_handler;
pub mod knowledge_base;
pub mod ai;

use anyhow::Result;
use command_handler::execute;

use tracing::debug;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

fn init_tracing() -> WorkerGuard {
    let file_appender = tracing_appender::rolling::hourly("./logs", "log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    tracing::subscriber::set_global_default(
        fmt::Subscriber::builder()
            // subscriber configuration
            .with_env_filter(EnvFilter::from_default_env())
            .finish()
            // add additional writers
            .with(fmt::Layer::default().with_writer(file_writer)),
    )
    .expect("Unable to set global tracing subscriber");
    debug!("Tracing initialized.");
    guard
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = init_tracing();
    execute().await?;
    Ok(())
}
