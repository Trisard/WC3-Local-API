use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;
use tungstenite::Message;

/// Configuration for the WebSocket connection.
pub struct ConnectionConfig {
    pub ports: Vec<u16>,
    pub path: String,
    /// Delay before reconnecting after a full port-cycle failure. Default: 5s.
    pub retry_delay: Duration,
    /// Timeout per port attempt. Default: 2s.
    pub connect_timeout: Duration,
}

impl ConnectionConfig {
    pub fn new(ports: Vec<u16>, path: impl Into<String>) -> Self {
        Self {
            ports,
            path: path.into(),
            retry_delay: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(2),
        }
    }

    pub fn with_retry_delay(mut self, d: Duration) -> Self {
        self.retry_delay = d;
        self
    }

    pub fn with_connect_timeout(mut self, d: Duration) -> Self {
        self.connect_timeout = d;
        self
    }
}

/// Lifecycle events emitted by the background connection thread.
#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connecting { port: u16 },
    Connected { port: u16 },
    Disconnected,
    Reconnecting,
}

/// Handle returned by [`connect`]. Holds the channels to receive messages and status events.
///
/// Dropping this handle (or calling [`shutdown`](ConnectionHandle::shutdown)) stops the
/// background thread.
pub struct ConnectionHandle {
    /// Raw text frames received from the WebSocket.
    pub messages: Receiver<String>,
    /// Connection lifecycle events.
    pub status: Receiver<ConnectionStatus>,
    // Dropping this sender signals the background thread to exit.
    _shutdown: Sender<()>,
}

impl ConnectionHandle {
    /// Blocking iterator over incoming messages.
    pub fn iter(&self) -> impl Iterator<Item = String> + '_ {
        self.messages.iter()
    }

    /// Stop the background thread immediately.
    pub fn shutdown(self) {
        drop(self._shutdown);
    }
}

/// Connect with an explicit config. Returns immediately; a background thread drives the socket.
pub fn connect(config: ConnectionConfig) -> ConnectionHandle {
    let (msg_tx, msg_rx) = mpsc::channel::<String>();
    let (status_tx, status_rx) = mpsc::channel::<ConnectionStatus>();
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    thread::spawn(move || {
        run_loop(config, msg_tx, status_tx, shutdown_rx);
    });

    ConnectionHandle {
        messages: msg_rx,
        status: status_rx,
        _shutdown: shutdown_tx,
    }
}

/// Automatic discovery + connect (Windows only). Fails immediately if WC3 is not running.
#[cfg(windows)]
pub fn connect_auto() -> crate::Result<ConnectionHandle> {
    let (ports, guid) = crate::discovery::discover()?;
    Ok(connect(ConnectionConfig::new(ports, format!("/webui-socket/{guid}"))))
}

/// Like [`connect_auto`], but polls until WC3 is found before connecting.
///
/// `poll_interval` controls how often the process list is re-checked.
/// The function blocks the calling thread until the game is detected.
#[cfg(windows)]
pub fn connect_when_ready(poll_interval: std::time::Duration) -> ConnectionHandle {
    loop {
        match connect_auto() {
            Ok(handle) => return handle,
            Err(_) => std::thread::sleep(poll_interval),
        }
    }
}

// ── Background thread ────────────────────────────────────────────────────────

/// Opens a TCP connection with timeout, then upgrades it to WebSocket.
fn try_connect_ws(
    port: u16,
    path: &str,
    timeout: Duration,
) -> crate::Result<tungstenite::WebSocket<TcpStream>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let stream = TcpStream::connect_timeout(&addr, timeout).map_err(tungstenite::Error::Io)?;
    let url = format!("ws://127.0.0.1:{port}{path}");
    let (ws, _) = tungstenite::client(url, stream).map_err(|e| {
        crate::Wc3Error::WebSocket(match e {
            tungstenite::HandshakeError::Failure(err) => err,
            tungstenite::HandshakeError::Interrupted(_) => tungstenite::Error::Io(
                std::io::Error::new(std::io::ErrorKind::WouldBlock, "handshake interrupted"),
            ),
        })
    })?;
    Ok(ws)
}

fn run_loop(
    config: ConnectionConfig,
    msg_tx: Sender<String>,
    status_tx: Sender<ConnectionStatus>,
    shutdown_rx: Receiver<()>,
) {
    'outer: loop {
        let mut connected = false;

        for &port in &config.ports {
            if shutdown_rx.try_recv().is_ok() {
                return;
            }

            let _ = status_tx.send(ConnectionStatus::Connecting { port });

            match try_connect_ws(port, &config.path, config.connect_timeout) {
                Ok(mut ws) => {
                    if status_tx.send(ConnectionStatus::Connected { port }).is_err() {
                        return;
                    }
                    connected = true;

                    loop {
                        if shutdown_rx.try_recv().is_ok() {
                            let _ = ws.close(None);
                            return;
                        }

                        match ws.read() {
                            Ok(Message::Text(text)) => {
                                if msg_tx.send(text.to_string()).is_err() {
                                    return;
                                }
                            }
                            Ok(Message::Close(_)) | Err(_) => break,
                            Ok(_) => {}
                        }
                    }

                    let _ = status_tx.send(ConnectionStatus::Disconnected);
                    // Reset to port 0 on disconnect
                    break;
                }
                Err(_) => continue,
            }
        }

        if shutdown_rx.try_recv().is_ok() {
            return;
        }

        if !connected {
            let _ = status_tx.send(ConnectionStatus::Reconnecting);
        }

        // Interruptible sleep: check shutdown every 100ms
        let steps = config.retry_delay.as_millis() / 100;
        for _ in 0..steps {
            thread::sleep(Duration::from_millis(100));
            if shutdown_rx.try_recv().is_ok() {
                return;
            }
        }

        continue 'outer;
    }
}
