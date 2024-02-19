use std::net::SocketAddr;
use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<()> {
    let listener: TcpListener = TcpListener::bind("127.0.0.1:4221").await?;
    loop {
        let (stream, addr) = listener.accept().await?;
        handle_connection(addr, stream).await?;
    }
}

async fn handle_connection(addr: SocketAddr, stream: TcpStream) -> Result<()> {
    println!("Accepted connection from {}", addr);
    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);

    let mut headers = Vec::new();
    let mut line_buffer = String::new();
    // Read until end of headers
    loop {
        reader.read_line(&mut line_buffer).await?;
        {
            let line_buffer = line_buffer.trim_end();
            if line_buffer.is_empty() {
                break;
            }

            headers.push(line_buffer.to_string());
        }
        line_buffer.clear();
    }

    let request_line = headers.get(0).context("Empty request?")?;
    let request_parts: Vec<_> = request_line.split_whitespace().collect();
    let method = *request_parts.get(0).context("Missing method")?;
    let path = *request_parts.get(1).context("Missing path")?;
    //let http_version = request_parts.get(2);

    println!("{method} '{path}'");

    match method {
        "GET" => {
            if path == "/" {
                writer.write(b"HTTP/1.1 200 OK\r\n\r\n").await?;
            } else {
                writer.write(b"HTTP/1.1 404 Not Found\r\n\r\n").await?;
            }
        }

        _ => _ = writer.write(b"HTTP/1.1 400 BAD_REQUEST\r\n\r\n").await?
    }


    writer.flush().await?;

    Ok(())
}
