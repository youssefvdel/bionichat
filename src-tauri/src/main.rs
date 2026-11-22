#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Held for the life of the process. A second copy exits here, before any
/// window or tunnel helper exists, so it can never duplicate the public link.
const SINGLE_INSTANCE_PORT: u16 = 8788;

fn main() {
    match std::net::TcpListener::bind(("127.0.0.1", SINGLE_INSTANCE_PORT)) {
        Ok(listener) => std::mem::forget(listener),
        Err(_) => {
            eprintln!("bionichat is already running; this copy will exit");
            std::process::exit(0);
        }
    }
    bionichat_lib::run();
}
