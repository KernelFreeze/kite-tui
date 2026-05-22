use clap::Parser;
use url::Url;

#[derive(Debug, Clone, Parser)]
#[command(name = "kite", version, about = "A terminal viewer for Kagi News")]
pub struct Args {
    #[arg(
        long,
        env = "KITE_BASE_URL",
        default_value = "https://news.kagi.com/",
        help = "Base URL for Kagi News data"
    )]
    pub base_url: Url,

    #[arg(short, long, help = "Initial category name or file stem")]
    pub category: Option<String>,

    #[arg(long, default_value_t = 20, help = "HTTP timeout in seconds")]
    pub timeout_seconds: u64,

    #[arg(
        long,
        env = "RUST_LOG",
        default_value = "kite=info,warn",
        help = "Tracing filter"
    )]
    pub log_filter: String,
}
