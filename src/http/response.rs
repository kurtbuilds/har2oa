use openapiv3 as oa;
use indexmap::indexmap;
use serde_json::Value;
use crate::http::Header;

#[derive(Debug)]
pub struct Response {
    pub data: Value,
    pub headers: Vec<Header>,
}

