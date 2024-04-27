use url::Url;
use crate::http::{Header, Query};

#[derive(Debug)]
pub struct Request {
    pub url: Url,
    pub headers: Vec<Header>,
    pub query: Vec<Query>,
    pub method: String,
}
