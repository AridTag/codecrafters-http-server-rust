mod http;

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::{bail, Context, Result};
use clap::Parser;
use once_cell::sync::Lazy;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::RwLock;
use crate::http::{FileContent, HttpResponse, HttpStatus, PlainTextContent};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long, default_value = None)]
    directory: Option<String>,
}

static CONFIG: Lazy<Arc<RwLock<Args>>> = Lazy::new(|| Arc::new(RwLock::new(Args::parse())));

#[tokio::main]
async fn main() -> Result<()> {
    let listener: TcpListener = TcpListener::bind("127.0.0.1:4221").await?;
    loop {
        let (stream, addr) = listener.accept().await?;
        tokio::spawn(handle_connection(addr, stream));
    }
}

#[derive(Debug)]
pub enum HttpMethod {
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

async fn read_headers(reader: &mut BufReader<OwnedReadHalf>) -> Result<HashMap<String, String>> {
    let mut line_buffer = String::new();
    let mut headers = HashMap::new();
    loop {
        reader.read_line(&mut line_buffer).await?;
        {
            let line_buffer = line_buffer.trim_end();
            if line_buffer.is_empty() {
                break;
            }

            let mut parts = line_buffer.splitn(2, ':');
            let key = parts.next().unwrap().trim().to_string();
            let value = parts.next().unwrap().trim().to_string();
            headers.insert(key, value);
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

async fn handle_connection(addr: SocketAddr, stream: TcpStream) {
    match handle_connection_inner(addr, stream).await {
        Ok(_) => {}
        Err(e) => eprintln!("Error handling connection from {}: {}", addr, e),
    }
}

async fn handle_connection_inner(addr: SocketAddr, stream: TcpStream) -> Result<()> {
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
pub struct RequestContext {
    pub reader: BufReader<OwnedReadHalf>,
    pub writer: BufWriter<OwnedWriteHalf>,
    pub method: HttpMethod,
    pub path: String,
    pub http_version: String,
    pub headers: HashMap<String, String>,
}

impl RequestContext {
    pub async fn send(&mut self, response: HttpResponse) -> Result<()> {
        self.writer.write(format!("HTTP/1.1 {} ", response.status() as u16).as_bytes()).await?;
        if let Some(message) = response.status_message() {
            self.writer.write(message.as_bytes()).await?;
        } else {
            self.writer.write(format!("{:?}", response.status()).as_bytes()).await?;
        }
        self.writer.write(b"\r\n").await?;

        for header in response.headers() {
            self.writer.write(format!("{}: {}\r\n", header.0, header.1).as_bytes()).await?;
        }
        if let Some(content) = response.content() {
            self.writer.write(format!("Content-Type: {}\r\n", content.content_type()).as_bytes()).await?;
            self.writer.write(format!("Content-Length: {}\r\n", content.content_length()).as_bytes()).await?;
        }
        self.writer.write(b"\r\n").await?;

        if let Some(content) = response.content().as_mut() {
            let mut content_reader = content.content()?;

            _ = tokio::io::copy(&mut content_reader, &mut self.writer).await?;
        }

        self.writer.flush().await?;
        Ok(())
    }
}

async fn process_request(mut ctx: RequestContext) -> Result<()> {
    println!("{} '{}'", ctx.method, ctx.path);

    let response = match ctx.method {
        HttpMethod::Get => {
            match ctx.path.as_str() {
                "/" => index(&mut ctx).await?,

                "/user-agent" => user_agent(&mut ctx).await?,

                path => {
                    if path.starts_with("/echo/") {
                        echo(&mut ctx).await?
                    } else if path.starts_with("/files/") {
                        files(&mut ctx).await?
                    } else {
                        HttpResponse::new(HttpStatus::NotFound)
                    }
                }
            }
        }

        HttpMethod::Post => {
            match ctx.path.as_str() {
                path => {
                    if path.starts_with("/files/") {
                        files_post(&mut ctx).await?
                    } else {
                        HttpResponse::new(HttpStatus::NotFound)
                    }
                }
            }
        }

        //_ => HttpResponse::new(HttpStatus::BadRequest)
    };

    ctx.send(response).await?;
    Ok(())
}

pub async fn index(_ctx: &mut RequestContext) -> Result<HttpResponse> {
    Ok(HttpResponse::new(HttpStatus::Ok))
}

pub async fn echo(ctx: &mut RequestContext) -> Result<HttpResponse> {
    let remaining = &ctx.path["/echo/".len()..];
    let content = PlainTextContent::new(remaining.to_string());
    Ok(HttpResponse::new(HttpStatus::Ok).with_content(content))
}

pub async fn user_agent(ctx: &mut RequestContext) -> Result<HttpResponse> {
    let agent = ctx.headers.get("User-Agent").cloned();
    let response = if let Some(agent) = agent {
        HttpResponse::new(HttpStatus::Ok)
            .with_content(PlainTextContent::new(agent))
    } else {
        HttpResponse::new(HttpStatus::BadRequest)
    };

    Ok(response)
}

pub async fn files(ctx: &mut RequestContext) -> Result<HttpResponse> {
    let file_path = {
        let config = CONFIG.read().await;
        if config.directory.is_none() {
            return Ok(HttpResponse::new(HttpStatus::InternalServerError));
        }

        PathBuf::from(config.directory.as_ref().unwrap()).join(&ctx.path["/files/".len()..])
    };

    let response = if !file_path.exists() {
        HttpResponse::new(HttpStatus::NotFound)
    } else {
        HttpResponse::new(HttpStatus::Ok).with_content(FileContent::new(file_path))
    };

    Ok(response)
}

pub async fn files_post(ctx: &mut RequestContext) -> Result<HttpResponse> {
    let dest_path = {
        let config = CONFIG.read().await;
        if config.directory.is_none() {
            return Ok(HttpResponse::new(HttpStatus::InternalServerError));
        }

        PathBuf::from(config.directory.as_ref().unwrap()).join(&ctx.path["/files/".len()..])
    };

    let content_length = {
        if let Some(content_length) = ctx.headers.get("Content-Length") {
            content_length.parse::<usize>()?
        } else {
            return Ok(HttpResponse::new(HttpStatus::BadRequest))
        }
    };

    let mut file = File::create(dest_path).await?;
    let mut bytes_read: usize = 0;
    let mut buf = vec![0; 8192];
    loop {
        let num_read = ctx.reader.read(&mut buf).await?;
        bytes_read += num_read;
        file.write_all(&buf[..num_read]).await?;
        if bytes_read >= content_length {
            break;
        }
    }

    Ok(HttpResponse::new(HttpStatus::Created))
}