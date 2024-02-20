use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use anyhow::{bail, Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

#[tokio::main]
async fn main() -> Result<()> {
    let listener: TcpListener = TcpListener::bind("127.0.0.1:4221").await?;
    loop {
        let (stream, addr) = listener.accept().await?;
        handle_connection(addr, stream).await?;
    }
}

#[derive(Debug)]
enum HttpMethod {
    Get,
    Post,
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Post => write!(f, "POST"),
        }
    }
}

impl TryFrom<&str> for HttpMethod {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        if value.eq_ignore_ascii_case("GET") {
            Ok(HttpMethod::Get)
        } else if value.eq_ignore_ascii_case("POST") {
            Ok(HttpMethod::Post)
        } else {
            bail!("HttpMethod {value} is not recognized")
        }
    }
}

async fn read_headers(reader: &mut BufReader<OwnedReadHalf>) -> Result<Vec<String>> {
    let mut line_buffer = String::new();
    let mut headers = Vec::new();
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

    Ok(headers)
}

async fn read_line(reader: &mut BufReader<OwnedReadHalf>) -> Result<String> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    Ok(line.trim().to_string())
}

async fn handle_connection(addr: SocketAddr, stream: TcpStream) -> Result<()> {
    println!("Accepted connection from {}", addr);
    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let writer = BufWriter::new(writer);

    let request_line = read_line(&mut reader).await?;
    let headers = read_headers(&mut reader).await?;

    let (method, path, http_version) = {
        let request_parts: Vec<_> = request_line.split_ascii_whitespace().collect();
        let method = HttpMethod::try_from(*request_parts.get(0).context("Missing method")?)?;
        let path = (*request_parts.get(1).context("Missing path")?).to_string();
        let http_version = match request_parts.get(2) {
            Some(ver) => (*ver).to_string(),
            _ => "HTTP/1.1".to_string()
        };

        (method, path, (*http_version).to_string())
    };

    let ctx = RequestContext {
        reader,
        writer,
        method,
        path,
        http_version,
        headers,
    };

    process_request(ctx).await?;

    Ok(())
}

#[allow(unused)]
struct RequestContext {
    pub reader: BufReader<OwnedReadHalf>,
    pub writer: BufWriter<OwnedWriteHalf>,
    pub method: HttpMethod,
    pub path: String,
    pub http_version: String,
    pub headers: Vec<String>,
}

async fn process_request(mut ctx: RequestContext) -> Result<()> {
    println!("{} '{}'", ctx.method, ctx.path);

    match ctx.method {
        HttpMethod::Get => {
            match ctx.path.as_str() {
                "/" => index(&mut ctx).await?,

                _ => {
                    if ctx.path.starts_with("/echo/") {
                        echo(&mut ctx).await?;
                    } else {
                        _ = ctx.writer.write(b"HTTP/1.1 404 Not Found\r\n\r\n").await?
                    }
                },
            }
        }

        _ => _ = ctx.writer.write(b"HTTP/1.1 400 BAD_REQUEST\r\n\r\n").await?
    }


    ctx.writer.flush().await?;
    Ok(())
}

async fn index(ctx: &mut RequestContext) -> Result<()> {
    ctx.writer.write(b"HTTP/1.1 200 OK\r\n\r\n").await?;
    Ok(())
}

async fn echo(ctx: &mut RequestContext) -> Result<()> {
    ctx.writer.write(b"HTTP/1.1 200 OK\r\n").await?;
    ctx.writer.write(b"Content-Type: text/plain\r\n").await?;
    let remaining = &ctx.path["/echo/".len()..];
    ctx.writer.write(format!("Content-Length: {}\r\n", remaining.len()).as_bytes()).await?;
    ctx.writer.write(b"\r\n").await?;

    ctx.writer.write(remaining.as_bytes()).await?;
    Ok(())
}
