use clap::Parser;
use kite_tui::{args::Args, error::Result};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing(&args.log_filter);

    kite_tui::run(args).await
}

fn init_tracing(filter: &str) {
    let env_filter =
        EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("kite=info,warn"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .init();
}
