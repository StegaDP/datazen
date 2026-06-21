#![forbid(unsafe_code)]

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(error) = frontend::run() {
        eprintln!("fatal error: {error}");
        std::process::exit(1);
    }
}
