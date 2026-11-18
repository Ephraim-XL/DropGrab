//! `dropgrabd`: the desktop/headless DropGrab backend (dropgrab-app-design.md sections 3.5, 4, 17).
//!
//! Starts the engine (database, identity, peer HTTPS listener, discovery) and the loopback control
//! API. The control API binds 127.0.0.1 only and needs the per-install bearer token, which is
//! written to `<data-dir>/api.token` (owner-only permissions).
//!
//! Flags (all optional):
//!   --data-dir <path>        default: $DROPGRAB_DATA_DIR, else ~/.local/share/dropgrab
//!   --downloads-dir <path>   default: ~/Downloads/DropGrab
//!   --control-port <port>    default: 53319 (section 3.5 loopback TCP port). 0 = ephemeral.
//!   --peer-port <port>       default: 53318. 0 = ephemeral (tests).
//!   --device-name <name>     default: hostname
//!   --no-discovery           disable multicast announce/listen
//!   --print-token            print the token to stdout (for scripts; otherwise only the file)
//!   --allow-origin <origin>  allow one browser Origin on the control API (repeatable).
//!                            Default: no browser origin is accepted (deny-all).
//!
//! Environment: DROPGRAB_ADVERTISE_IP overrides the address written into announcements and QR codes.

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use dg_core::{api, engine::Config, Backend};

struct Args {
    data_dir: PathBuf,
    downloads_dir: PathBuf,
    control_port: u16,
    peer_port: Option<u16>,
    device_name: Option<String>,
    discovery: bool,
    print_token: bool,
    allowed_origins: Vec<String>,
}

fn usage() -> ! {
    eprintln!(
        "usage: dropgrabd [--data-dir P] [--downloads-dir P] [--control-port N] [--peer-port N] \
         [--device-name S] [--no-discovery] [--print-token] [--allow-origin ORIGIN]..."
    );
    std::process::exit(2);
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// Validates an `--allow-origin` value: `scheme://host[:port]` with no path, which is exactly
/// what a browser puts in the `Origin` header. Exits with the usage message otherwise.
fn valid_origin(raw: &str) -> String {
    let o = api::normalize_origin(raw);
    let host = o.strip_prefix("http://").or_else(|| o.strip_prefix("https://"));
    match host {
        Some(h) if !h.is_empty() && !h.contains('/') => o,
        _ => {
            eprintln!("dropgrabd: --allow-origin needs an origin such as http://localhost:8080 (got {raw:?})");
            std::process::exit(2);
        }
    }
}

fn parse_args() -> Args {
    let mut a = Args {
        data_dir: dg_core::default_data_dir(),
        downloads_dir: home().join("Downloads").join("DropGrab"),
        control_port: 53319,
        peer_port: None,
        device_name: None,
        discovery: true,
        print_token: false,
        allowed_origins: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| usage());
        // `--allow-origin=URL` and `--allow-origin URL` both work.
        if let Some(rest) = flag.strip_prefix("--allow-origin=") {
            a.allowed_origins.push(valid_origin(rest));
            continue;
        }
        match flag.as_str() {
            "--data-dir" => a.data_dir = PathBuf::from(val()),
            "--downloads-dir" => a.downloads_dir = PathBuf::from(val()),
            "--control-port" => a.control_port = val().parse().unwrap_or_else(|_| usage()),
            "--peer-port" => a.peer_port = Some(val().parse().unwrap_or_else(|_| usage())),
            "--device-name" => a.device_name = Some(val()),
            "--allow-origin" => a.allowed_origins.push(valid_origin(&val())),
            "--no-discovery" => a.discovery = false,
            "--print-token" => a.print_token = true,
            _ => usage(),
        }
    }
    a
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = parse_args();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    if let Err(e) = rt.block_on(run(args)) {
        eprintln!("dropgrabd: {e}");
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<(), String> {
    let mut cfg = Config::desktop(args.data_dir.clone(), args.downloads_dir.clone());
    if let Some(n) = args.device_name.clone() {
        cfg.device_name = n;
    }
    if let Some(p) = args.peer_port {
        cfg.peer_port = p;
    }
    cfg.discovery = args.discovery;
    if let Ok(ip) = std::env::var("DROPGRAB_ADVERTISE_IP") {
        cfg.advertise_ip = ip.parse::<IpAddr>().ok();
    }

    let backend = Backend::start(cfg).await?;
    let token = api::load_or_create_token(&args.data_dir).map_err(|e| e.to_string())?;
    let (addr, server) =
        api::serve_with_origins(backend.engine.clone(), args.control_port, token.clone(), args.allowed_origins.clone()).await?;

    eprintln!(
        "dropgrabd: control API http://{addr} (token file: {})",
        args.data_dir.join(api::TOKEN_FILE).display()
    );
    eprintln!("dropgrabd: peer HTTPS port {}", backend.engine.peer_port);
    if args.print_token {
        println!("{token}");
    }

    // Run until Ctrl-C or a `backend.shutdown` command sets `stopping`.
    let engine = backend.engine.clone();
    tokio::select! {
        _ = tokio::signal::ctrl_c() => eprintln!("dropgrabd: interrupt received"),
        _ = async {
            while !engine.stopping.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        } => eprintln!("dropgrabd: shutdown requested"),
    }
    server.abort();
    backend.stop().await;
    eprintln!("dropgrabd: stopped");
    Ok(())
}
