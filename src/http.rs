use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use nom::ToUsize;
use tokio::io::{AsyncRead, BufReader};

#[allow(unused)]
#[derive(Copy, Clone, Debug)]
pub enum HttpStatus {
    Ok = 200,
    Created = 201,
    BadRequest = 400,
    NotFound = 404,
    InternalServerError = 500,
}

impl Into<&'static str> for HttpStatus {
    fn into(self) -> &'static str {
        match self {
            HttpStatus::Ok => "OK",
            HttpStatus::Created => "Created",
            HttpStatus::BadRequest => "BadRequest",
            HttpStatus::NotFound => "NotFound",
            HttpStatus::InternalServerError => "InternalServerError",
        }
    }
}

pub struct HttpResponse {
    status: HttpStatus,
    status_message: Option<String>,
    headers: HashMap<String, String>,
    content: Option<Box<dyn HttpContent + Send + Sync>>,
}

impl HttpResponse {
    pub fn new(status: HttpStatus) -> Self {
        Self {
            status,
            status_message: None,
            headers: HashMap::new(),
            content: None,
        }
    }

    pub fn with_status_message(self, message: String) -> Self {
        Self {
            status: self.status,
            status_message: Some(message),
            headers: self.headers,
            content: self.content,
        }
    }

    pub fn with_content(self, content: Box<dyn HttpContent + Send + Sync>) -> Self {
        Self {
            status: self.status,
            status_message: self.status_message,
            headers: self.headers,
            content: Some(content),
        }
    }

    pub fn status(&self) -> HttpStatus {
        self.status
    }

    pub fn status_message(&self) -> Option<&String> {
        self.status_message.as_ref()
    }

    pub fn content(&self) -> Option<&Box<dyn HttpContent + Send + Sync>> {
        self.content.as_ref()
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
}

pub trait HttpContent {
    fn content_type(&self) -> &str;
    fn content_length(&self) -> usize;
    fn content(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin + '_>, anyhow::Error>;
}

pub struct PlainTextContent {
    text: String
}

impl PlainTextContent {
    pub fn new(text: String) -> Box<Self> {
        Box::new(Self { text })
    }
}

impl HttpContent for PlainTextContent {
    fn content_type(&self) -> &str {
        "text/plain"
    }

    fn content_length(&self) -> usize {
        self.text.len()
    }

    fn content(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin + '_>, anyhow::Error> {
        let cursor = std::io::Cursor::new(self.text.as_bytes());
        Ok(Box::new(cursor))
    }
}

pub struct FileContent {
    path: PathBuf,
}

impl FileContent {
    pub fn new(path: PathBuf) -> Box<Self> {
        Box::new(Self { path })
    }
}

impl HttpContent for FileContent {
    fn content_type(&self) -> &str {
        "application/octet-stream"
    }

    fn content_length(&self) -> usize {
        fs::metadata(self.path.as_path()).expect("File doesn't exist?").len().to_usize()
    }

    fn content(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin + '_>, anyhow::Error> {
        let file = File::open(&self.path)?;
        let file = tokio::fs::File::from(file);
        Ok(Box::new(BufReader::new(file)))
    }
}