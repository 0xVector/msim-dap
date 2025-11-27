use super::Result;
use std::net::TcpStream;
use std::time::Duration;

/// Connects to the given port and returns the stream
pub fn connect(port: u16) -> Result<TcpStream> {
    let s = TcpStream::connect(("127.0.0.1", port))?;
    s.set_nodelay(true)?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    s.set_write_timeout(Some(Duration::from_secs(5)))?;
    eprintln!("Connected to MSIM with port {}", port);
    Ok(s)
}