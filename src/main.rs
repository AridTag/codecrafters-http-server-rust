use std::net::SocketAddr;
use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<()> {
    let listener: TcpListener = TcpListener::bind("127.0.0.1:4221").await?;
    loop {
        let (stream, addr) = listener.accept().await?;
        handle_connection(addr, stream).await?;
    }
}

async fn handle_connection(addr: SocketAddr, mut stream: TcpStream) -> Result<()> {
    println!("Accepted connection from {}", addr);
    let mut buffer = vec![0u8; 8192];
    let _bytes_read = stream.read(&mut buffer).await?;

    stream.write(b"HTTP/1.1 200 OK\r\n\r\n").await?;
    stream.flush().await?;

    Ok(())
}
