//! bionichat's own OpenTunnel client.
//!
//! The upstream `opentunnel` CLI refuses to build outside Unix, because its
//! background service talks over a Unix socket. The library underneath is
//! portable, so this binary does the only two things bionichat needs, with no
//! service manager and no sockets:
//!
//! * print the tunnel's public URL, then serve it in the foreground
//! * `--revoke` to delete the tunnel server-side, so the next start mints a
//!   new hostname
//!
//! The hostname is stable per machine because it comes from the stored
//! identity in `$XDG_DATA_HOME/opentunnel/<profile>/tunnel.json`.

use std::collections::BTreeMap;
use std::io::{self, Write};

use anyhow::{Context, Result};
use std::io::IsTerminal as _;
use opentunnel::{Client, ClientOptions, Route, Routes};

struct Args {
    port: u16,
    profile: String,
    revoke: bool,
}

fn parse_args() -> Result<Args> {
    let mut port = 8787_u16;
    let mut profile = "default".to_string();
    let mut revoke = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                port = args
                    .next()
                    .context("--port needs a number")?
                    .parse()
                    .context("--port must be a number")?
            }
            "--profile" => profile = args.next().context("--profile needs a name")?,
            "--revoke" => revoke = true,
            other => anyhow::bail!("unknown argument `{other}`"),
        }
    }
    Ok(Args {
        port,
        profile,
        revoke,
    })
}

fn main() -> Result<()> {
    let args = parse_args()?;

    // The library resolves its identity directory from HOME. Windows sets
    // USERPROFILE instead, and a process started by the Task Scheduler may
    // have neither: without this the helper would provision a fresh tunnel in
    // the temp directory and lose the machine's stable hostname.
    if std::env::var_os("HOME").is_none() {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            // SAFETY: nothing else runs yet, and this is the process start.
            unsafe { std::env::set_var("HOME", profile) };
        }
    }
    let client = Client::new(ClientOptions {
        api: None,
        storage: None,
        ca_cert: None,
    });

    // Two helpers on one identity break the tunnel (the server sees the same
    // tunnel register twice). A listening socket is the cheapest cross-platform
    // lock: binding the same port twice fails.
    let lock = std::net::TcpListener::bind(("127.0.0.1", args.port + 2));
    match lock {
        Ok(listener) => {
            std::mem::forget(listener);
        }
        Err(_) => {
            eprintln!(
                "another bionichat tunnel is already running for port {}",
                args.port
            );
            std::process::exit(3);
        }
    }

    // Exit with the app that started us. The app keeps our stdin pipe open;
    // when it dies the pipe ends and this thread ends the helper, so no orphan
    // is left holding the tunnel lock.
    if !std::io::stdin().is_terminal() {
        std::thread::spawn(|| {
            use std::io::Read as _;
            let mut sink = [0u8; 64];
            let mut stdin = std::io::stdin();
            loop {
                match stdin.read(&mut sink) {
                    Ok(0) => std::process::exit(0),
                    Ok(_) => {}
                    Err(_) => std::process::exit(0),
                }
            }
        });
    }

    let runtime = tokio::runtime::Runtime::new().context("starting the async runtime")?;
    runtime.block_on(async move {
        if args.revoke {
            client
                .remove(&args.profile)
                .await
                .context("deleting the tunnel on the server")?;
            println!("revoked {}", args.profile);
            return Ok(());
        }

        let identity = client
            .ensure(&args.profile)
            .await
            .context("provisioning the tunnel")?;
        // The app scrapes this line; keep it plain and flush it immediately.
        println!("https://{}", identity.hostname);
        io::stdout().flush().ok();

        let mut routes: Routes = BTreeMap::new();
        // `@` is the tunnel's root route: the bare hostname the app prints.
        routes.insert("@".to_string(), Route::new(format!("127.0.0.1:{}", args.port)));
        let _tunnel = client
            .connect(&args.profile, routes)
            .context("opening the tunnel")?;
        eprintln!("tunnel: serving 127.0.0.1:{} as {}", args.port, identity.hostname);

        // Serve until the app kills us.
        std::future::pending::<()>().await;
        Ok(())
    })
}
