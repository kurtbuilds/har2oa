use serde_json::Value;
use url::Url;
use crate::http::{Header, Query};

#[derive(Debug)]
pub struct Request {
    pub url: Url,
    pub headers: Vec<Header>,
    pub query: Vec<Query>,
    pub method: String,
    pub body: Option<RequestBody>,
}

#[derive(Debug)]
pub struct RequestBody {
    pub mime: String,
    pub content: Value,
}
