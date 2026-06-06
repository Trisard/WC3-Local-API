use wc3_local_api::{connect_when_ready, ConnectionStatus};
use std::time::Duration;

fn main() {
    eprintln!("Waiting for Warcraft III...");
    let handle = connect_when_ready(Duration::from_secs(3));

    // Forward status events from a separate thread so they don't block the message loop.
    let status_rx = handle.status;
    std::thread::spawn(move || {
        for s in status_rx {
            match s {
                ConnectionStatus::Connecting { port } => eprintln!("[connecting] port {port}"),
                ConnectionStatus::Connected { port }  => eprintln!("[connected]  port {port}"),
                ConnectionStatus::Disconnected        => eprintln!("[disconnected]"),
                ConnectionStatus::Reconnecting        => eprintln!("[reconnecting in 5s...]"),
            }
        }
    });

    for msg in handle.messages {
        println!("{msg}");
    }
}
