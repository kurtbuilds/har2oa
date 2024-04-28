use openapiv3 as oa;
mod request;
mod response;

use anyhow::Result;
use convert_case::{Case, Casing};
use serde_json::Value;
use url::Url;
use har::Spec;
use har::v1_2::Entries;
use indexmap::indexmap;
use crate::openapi::extract_object_name;
pub use request::*;
pub use response::*;

pub type Header = (String, String);
pub type Query = (String, String);

#[derive(Debug)]
pub struct RequestResponse {
    pub info: RequestInfo,
    pub request: Request,
    pub response: Response,
}

impl RequestResponse {
    pub fn method(&self) -> &str {
        &self.info.method
    }

    pub fn path(&self) -> &str {
        &self.info.path
    }

    pub fn operation_id(&self) -> &str {
        &self.info.operation_id
    }

    pub fn object_name(&self) -> &str {
        &self.info.object_name
    }

    pub fn response_object_name(&self) -> &str {
        &self.info.response_object_name
    }

    pub fn response_schema_ref(&self) -> oa::Response {
        oa::Response {
            headers: indexmap! {},
            content: indexmap! {
                "application/json".to_string() => oa::MediaType {
                    schema: Some(oa::ReferenceOr::schema_ref(&self.info.response_object_name)),
                    ..oa::MediaType::default()
                },
            },
            ..oa::Response::default()
        }
    }
}

#[derive(Debug)]
pub enum ParameterType {
    Integer,
}

#[derive(Debug)]
pub struct PathParameter {
    pub name: String,
    pub typ: ParameterType,
}

#[derive(Debug)]
pub struct RequestInfo {
    pub path: String,
    pub object_name: String,
    pub operation_id: String,
    pub response_object_name: String,
    pub method: String,
    /// The path parameters, e.g. `["id"]` for `/users/{id}`
    pub path_parameters: Vec<PathParameter>,
}

fn pluralize(s: &mut String) {
    if s.ends_with("ss") {
        s.push_str("es")
    } else if s.ends_with("y") {
        s.pop();
        s.push_str("ies")
    } else if !s.ends_with("s") {
        s.push_str("s")
    }
}

pub fn singular(s: &str) -> String {
    if s.ends_with("sses") {
        s[..s.len() - 2].to_string()
    } else if s.ends_with("ies") {
        s[..s.len() - 3].to_string() + "y"
    } else if s.ends_with("s") && !s.ends_with("ss") {
        s[..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn plural(s: &str) -> bool {
    s.ends_with("s") && !s.ends_with("ss")
}

impl RequestInfo {
    fn from_request(request: &Request) -> Self {
        // As in, fetch one or fetch many
        let mut gets_many = false;
        let mut path_parameters = Vec::new();

        let mut path_segments = request.url.path()
            .split("/")
            .skip(2)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .map(|s| {
                if s.chars().all(char::is_numeric) {
                    path_parameters.push(PathParameter {
                        name: "id".to_string(),
                        typ: ParameterType::Integer,
                    });
                    "{id}".to_string()
                } else {
                    s
                }
            }).peekable();

        let mut path = String::new();
        let mut operation_id = request.method.to_case(Case::Snake);

        let mut object_name = None;

        while let Some(segment) = path_segments.next() {
            path.push('/');
            path.push_str(&segment);
            // let last = path_segments.peek().is_none();
            if segment == "list" || segment == "all" {
                gets_many = true;
            } else if segment.ends_with("list") && !plural(&segment[..segment.len() - 4]) {
                gets_many = true;
                let segment = &segment[..segment.len() - 4];
                operation_id.push_str(&segment.to_case(Case::Pascal));
                object_name = Some(segment.to_string());
            } else if !segment.starts_with("{") {
                operation_id.push_str(&segment.to_case(Case::Pascal));
                object_name = Some(segment);
            }
        }
        if gets_many {
            pluralize(&mut operation_id);
        }
        let object_name = object_name.expect(&format!("No object name found in path {:?}", &request.url.as_str()));

        let object_name = extract_object_name(&object_name).unwrap().to_case(Case::Pascal);
        let mut response_object_name = operation_id.to_case(Case::Pascal);
        response_object_name.push_str("Response");
        Self {
            path,
            path_parameters,
            object_name,
            operation_id,
            response_object_name,
            method: request.method.clone(),
        }
    }
}

fn ignore_header(h: &str) -> bool {
    [
        "content-length",
        "content-type",
        "accept",
        "user-agent",
        "authorization",
        "accept-encoding",
        "accept-language",
        "access-control-allow-methods",
        "access-control-allow-origin",
        "access-control-allow-headers",
        "access-control-allow-credentials",
        "access-control-expose-headers",
        "server",
        "date",
        ":authority",
        ":method",
        ":path",
        ":scheme",
        "cookie",
        "dnt",
        "referer",
        "sec-ch-ua",
        "sec-ch-ua-mobile",
        "sec-ch-ua-platform",
        "sec-fetch-dest",
        "sec-fetch-mode",
        "sec-fetch-site",
    ].contains(&h)
}

impl From<Entries> for RequestResponse {
    fn from(entry: Entries) -> Self {
        let mut request = Request {
            url: Url::parse(&entry.request.url).unwrap(),
            headers: entry.request.headers.into_iter()
                .map(|h| (h.name, h.value))
                .filter(|(h, _)| !ignore_header(h))
                .collect(),
            query: entry.request.query_string
                .into_iter()
                .map(|h| (urlparse::unquote(h.name).unwrap(), h.value))
                .collect(),
            method: entry.request.method,
            body: entry.request.post_data
                .map(|pd| {
                    let mime = pd.mime_type;
                    let content = if let Some(text) = pd.text {
                        if let Ok(json) = serde_json::from_str(&text) {
                            json
                        } else {
                            Value::String(text)
                        }
                    } else {
                        Value::Null
                    };
                    RequestBody {
                        mime,
                        content,
                    }
                })
        };
        let data = match entry.response.content.text {
            Some(text) => {
                serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text))
            },
            None => Value::Null,
        };
        let mut response = Response {
            data,
            headers: entry.response.headers
                .into_iter()
                .map(|h| (h.name, h.value))
                .filter(|(h, _)| !ignore_header(h))
                .collect(),
        };
        let info = RequestInfo::from_request(&request);
        RequestResponse {
            info,
            request,
            response,
        }
    }
}


pub fn read_har(path: &str) -> Result<Vec<Entries>> {
    let har = har::from_path(path)?;
    let entries = match har.log {
        Spec::V1_2(har::v1_2::Log { entries, .. }) => entries,
        Spec::V1_3(har::v1_3::Log { .. }) => unimplemented!(),
    };

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use openapiv3 as oa;

    #[test]
    fn test_into_rr_client() -> Result<()> {
        let filter = vec!["https://app.studiodesigner.com/api/".to_string()];
        let rr = read_har("data/app.studiodesigner.com/api/clients.har")?
            .into_iter()
            .filter(|e| e.request.url.starts_with("https://app.studiodesigner.com/api/"))
            .next()
            .map(RequestResponse::from)
            .unwrap();
        assert_eq!(rr.operation_id(), "getClients");
        assert_eq!(rr.object_name(), "Client");
        assert_eq!(rr.response_object_name(), "GetClientsResponse");
        Ok(())
    }


    #[test]
    fn test_into_rr_employeeslist() -> Result<()> {
        let filter = vec!["https://app.studiodesigner.com/api/".to_string()];
        let rr = read_har("data/app.studiodesigner.com/api/employeelist.har")?
            .into_iter()
            .filter(|e| e.request.url.starts_with("https://app.studiodesigner.com/api/"))
            .next()
            .map(RequestResponse::from)
            .unwrap();
        assert_eq!(rr.operation_id(), "getEmployees");
        assert_eq!(rr.object_name(), "Employee");
        assert_eq!(rr.response_object_name(), "GetEmployeesResponse");
        Ok(())
    }
}