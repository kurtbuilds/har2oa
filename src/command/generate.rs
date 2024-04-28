use crate::http::{read_har, RequestResponse};
use crate::openapi;
use crate::openapi::{operation, response};
use anyhow::Result;
use clap::Args;
use har::v1_2::Entries;
use indexmap::indexmap;
use itertools::Itertools;
use openapiv3 as oa;
use openapiv3::ReferenceOr;
use std::fs;
use tracing::debug;

#[derive(Debug, Args)]
pub struct Generate {
    pub har_file: String,
    /// Add cookie authentication. E.g. `--cookie sessid` adds a security schema where a cookie named `sessid` is required
    #[clap(long)]
    pub cookie: Option<String>,
    #[clap(short, long)]
    pub output: Option<String>,
}

impl Generate {
    pub fn run(self) -> Result<()> {
        let hars: Vec<Entries> = read_har(&self.har_file)?;
        let mut rrs: Vec<RequestResponse> = hars
            .into_iter()
            .map(|h| h.into())
            // .unique_by(|rr: &RequestResponse| rr.request.url.path().to_string())
            .collect::<Vec<_>>();

        rrs.sort_by_key(|rr| rr.request.url.path().to_string());
        debug!(n = rrs.len(), "Read har requests");

        let server = {
            let urls = rrs
                .iter()
                .map(|rr| rr.request.url.as_str())
                .collect::<Vec<_>>();
            let mut server = longest_common_prefix(&urls);
            if server.ends_with('/') {
                server.truncate(server.len() - 1);
            }
            server
        };
        let security_schema_name = "Session".to_string();
        let mut schema = oa::OpenAPI {
            openapi: "3.0.3".to_string(),
            info: Default::default(),
            servers: vec![oa::Server {
                url: server,
                description: None,
                variables: Default::default(),
                extensions: Default::default(),
            }],
            paths: Default::default(),
            components: oa::Components::default(),
            security: Vec::new(),
            tags: vec![],
            external_docs: None,
            extensions: Default::default(),
        };

        response::create_schema_for_responses(&rrs, &mut schema.components)?;
        operation::create_paths(&rrs, &mut schema.paths);

        if let Some(cookie) = self.cookie {
            schema.security = vec![indexmap! {
                security_schema_name.clone() => vec![security_schema_name.clone()],
            }];

            schema.security_schemes.insert(
                security_schema_name,
                ReferenceOr::Item(oa::SecurityScheme::APIKey {
                    location: oa::APIKeyLocation::Cookie,
                    name: cookie,
                    description: None,
                }),
            );
        };

        let s = serde_yaml::to_string(&schema)?;
        let path = self.output.as_deref().unwrap_or("openapi.yaml");
        // let path = "openapi.yaml";
        fs::write(path, &s)?;
        println!("{}: Wrote file.", path);
        Ok(())
    }
}

fn longest_common_prefix(strings: &[&str]) -> String {
    if strings.is_empty() {
        return String::new();
    }

    // Get the first string as a starting point for comparison
    let mut prefix = strings[0].to_string();

    for &s in strings.iter().skip(1) {
        let mut i = 0;
        while i < prefix.len() && i < s.len() && prefix.as_bytes()[i] == s.as_bytes()[i] {
            i += 1;
        }
        if i < prefix.len() {
            prefix.truncate(i);
        }
    }
    prefix
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_longest_common_prefix() {
        let strings = vec![
            "https://app.studiodesigner.com/api/item/details/12334",
            "https://app.studiodesigner.com/api/itemlist",
            "https://app.studiodesigner.com/api/activities/list",
        ];
        assert_eq!(
            longest_common_prefix(&strings),
            "https://app.studiodesigner.com/api/"
        );
    }
}
