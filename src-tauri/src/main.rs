#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

enum Mode {
    Gui,
    Cli,
    Server {
        bind: std::net::IpAddr,
        port: u16,
        token: Option<String>,
    },
}

struct Flags {
    cli: bool,
    server: Option<(std::net::IpAddr, u16, Option<String>)>,
    port: Option<u16>,
    bind: Option<std::net::IpAddr>,
    token: Option<String>,
}

fn parse_args() -> Flags {
    let args: Vec<String> = std::env::args().collect();
    let mut f = Flags { cli: false, server: None, port: None, bind: None, token: None };
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--cli" | "-c" => f.cli = true,
            "--server" => f.server = Some((std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 7333, None)),
            s if s.starts_with("--port=") || s == "--port" => {
                let v = value_for(&args, &mut i, s, "--port=");
                match v.parse::<u16>() {
                    Ok(p) => f.port = Some(p),
                    Err(_) => {
                        eprintln!("Invalid --port value: {v}");
                        std::process::exit(2);
                    }
                }
            }
            s if s.starts_with("--bind=") || s == "--bind" => {
                let v = value_for(&args, &mut i, s, "--bind=");
                match v.parse::<std::net::IpAddr>() {
                    Ok(b) => f.bind = Some(b),
                    Err(_) => {
                        eprintln!("Invalid --bind value: {v}");
                        std::process::exit(2);
                    }
                }
            }
            s if s.starts_with("--token=") || s == "--token" => {
                f.token = Some(value_for(&args, &mut i, s, "--token="));
            }
            other => {
                eprintln!("Unknown argument: {other}");
                print_help();
                std::process::exit(2);
            }
        }
        i += 1;
    }
    if f.server.is_some() {
        let (b, p, tk) = f.server.take().unwrap();
        f.server = Some((f.bind.unwrap_or(b), f.port.unwrap_or(p), f.token.clone()));
    }
    f
}

/// `--flag=value` inline, or `--flag value` with the value in the next arg.
fn value_for(args: &[String], i: &mut usize, cur: &str, prefix: &str) -> String {
    if let Some(v) = cur.strip_prefix(prefix) {
        if !v.is_empty() {
            return v.to_string();
        }
    }
    if *i + 1 < args.len() {
        *i += 1;
        return args[*i].clone();
    }
    eprintln!("Missing value for {prefix}...");
    std::process::exit(2);
}

fn print_help() {
    println!("Monitor - Container & Host Monitor\n");
    println!("Usage: monitor [FLAGS]\n");
    println!("Flags:");
    println!("      (none)          Launch the desktop GUI");
    println!("  --cli, -c           Run the terminal UI (btop-style)");
    println!("  --server            Serve the web UI over HTTP (browser access)");
    println!("  --port=<n>          Port for --server (default 7333)");
    println!("  --bind=<addr>       Bind address for --server (default 0.0.0.0)");
    println!("  --token=<t>         Require ?token=<t> on /ws and API calls");
    println!("  -h, --help          Print this help");
}

pub fn main() {
    let flags = parse_args();
    let mode = if flags.server.is_some() {
        let (bind, port, token) = flags.server.unwrap();
        Mode::Server { bind, port, token }
    } else if flags.cli {
        Mode::Cli
    } else {
        Mode::Gui
    };
    match mode {
        Mode::Gui => monitor_lib::run(),
        Mode::Cli => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to start tokio runtime");
            rt.block_on(monitor_lib::run_cli());
        }
        Mode::Server { bind, port, token } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to start tokio runtime");
            rt.block_on(monitor_lib::run_server(bind, port, token));
        }
    }
}
