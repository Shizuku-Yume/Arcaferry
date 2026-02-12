use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    // Lightweight CLI flags (no external deps).
    // Usage (cargo): cargo run --bin server -- --sidecar-headed --sidecar-trace
    // Usage (binary): ./server --sidecar-headed --sidecar-trace
    let mut sidecar_headed = false;
    let mut sidecar_trace = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--sidecar-headed" => sidecar_headed = true,
            "--sidecar-trace" | "--sidecar-verbose" => sidecar_trace = true,
            _ => {}
        }
    }
    if sidecar_headed {
        std::env::set_var("ARCAFERRY_SIDECAR_HEADED", "1");
    }
    if sidecar_trace {
        std::env::set_var("ARCAFERRY_SIDECAR_TRACE", "1");
    }

    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("arcaferry_lib=debug,info")),
        )
        .init();

    let port = std::env::var("ARCAFERRY_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(17236);

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async move {
        if let Err(e) = arcaferry_lib::server::start_server(port).await {
            eprintln!("server error: {}", e);
            std::process::exit(1);
        }
    });
}
