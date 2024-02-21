use std::collections::HashMap;
use tokio::io::AsyncRead;

#[allow(unused)]
#[derive(Copy, Clone, Debug)]
pub enum HttpStatus {
    Ok = 200,
    BadRequest = 400,
    NotFound = 404,
}

impl Into<&'static str> for HttpStatus {
    fn into(self) -> &'static str {
        match self {
            HttpStatus::Ok => "OK",
            HttpStatus::BadRequest => "BadRequest",
            HttpStatus::NotFound => "NotFound",
        }
    }
}

pub struct HttpResponse {
    status: HttpStatus,
    status_message: Option<String>,
    headers: HashMap<String, String>,
    content: Option<Box<dyn HttpContent>>,
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

    pub fn with_content(self, content: Box<dyn HttpContent>) -> Self {
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

    pub fn content(&self) -> Option<&Box<dyn HttpContent>> {
        self.content.as_ref()
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
}

pub trait HttpContent {
    fn content_type(&self) -> &str;
    fn content_length(&self) -> u64;
    fn content(&self) -> Box<dyn AsyncRead + Unpin + '_>;
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

    fn content_length(&self) -> u64 {
        self.text.len().try_into().expect("Text too big!")
    }

    fn content(&self) -> Box<dyn AsyncRead + Unpin + '_> {
        let cursor = std::io::Cursor::new(self.text.as_bytes());
        Box::new(cursor)
    }
}