use crate::http::RequestResponse;
use crate::openapi;
use anyhow::Result;
use openapiv3 as oa;
use std::collections::HashSet;
use tracing::{info, warn};

/// Root function that takes a response and attaches its data to the OpenAPI object.
pub fn create_schema_for_responses(
    rrs: &[RequestResponse],
    components: &mut oa::Components,
) -> Result<()> {
    let mut seen = HashSet::new();
    for rr in rrs {
        seen.insert(rr.request.url.path().to_string());
        // .unique_by(|rr: &RequestResponse| rr.request.url.path().to_string())

        info!(url = rr.request.url.as_str(), "Analyzing req/res");
        if let Err(e) = openapi::add_response_schemas(components, &rr) {
            warn!(url=rr.request.url.as_str(), err=?e, "Error adding schemas for url");
            continue;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::http::read_har;

    #[test]
    fn test_into_rr_activities_list() -> Result<()> {
        let rr = read_har("data/app.studiodesigner.com/api/activities/list.har")?
            .into_iter()
            .filter(|e| {
                e.request
                    .url
                    .starts_with("https://app.studiodesigner.com/api/")
            })
            .next()
            .map(RequestResponse::from)
            .unwrap();
        assert_eq!(rr.operation_id(), "getActivities");
        assert_eq!(rr.object_name(), "Activity");
        assert_eq!(rr.response_object_name(), "GetActivitiesResponse");
        let mut schema = oa::OpenAPI::default();
        let rrs = vec![rr];
        create_schema_for_responses(&rrs, &mut schema.components)?;
        let schema = schema
            .schemas
            .get("GetActivitiesResponse")
            .unwrap()
            .as_item()
            .unwrap();
        assert!(matches!(
            schema.kind,
            oa::SchemaKind::Type(oa::Type::Object(_))
        ));
        Ok(())
    }
}
