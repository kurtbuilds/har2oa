use crate::http::{ParameterType, RequestBody, RequestResponse};
use crate::openapi;
use crate::openapi::parameter;
use indexmap::indexmap;
use itertools::Itertools;
use openapiv3 as oa;
use openapiv3::{RefOr, ReferenceOr};
use std::str::FromStr;

pub fn make_body(body: &Option<RequestBody>) -> Option<RefOr<oa::RequestBody>> {
    let body = body.as_ref()?;
    if !body.mime.starts_with("application/json") {
        return None;
    };
    let obj = body.content.as_object()?;
    let mut schema = oa::Schema::new_object();
    let mut props = schema.properties_mut();
    for (key, value) in obj {
        let name = key.to_string();
        if value.is_number() {
            props.insert(name, oa::Schema::new_number());
        } else if value.is_boolean() {
            props.insert(name, oa::Schema::new_bool());
        } else if value.is_string() {
            props.insert(name, oa::Schema::new_string());
        }
    }
    Some(RefOr::Item(oa::RequestBody {
        content: indexmap! {
            "application/json".to_string() => oa::MediaType {
                schema: Some(RefOr::Item(schema)),
                ..oa::MediaType::default()
            },
        },
        required: true,
        ..oa::RequestBody::default()
    }))
}

pub fn create_operation(rr: &RequestResponse) -> anyhow::Result<oa::Operation> {
    let response = rr.response_schema_ref();
    let mut parameters = rr
        .request
        .query
        .iter()
        .unique_by(|(key, _)| key)
        .map(|(key, value)| parameter::create_parameter(key, value))
        .collect::<anyhow::Result<Vec<_>, anyhow::Error>>()?
        .into_iter()
        .map(|p| ReferenceOr::Item(p))
        .collect::<Vec<_>>();
    for param in &rr.info.path_parameters {
        let format = match param.typ {
            ParameterType::Integer => RefOr::Item(oa::Schema::new_integer()),
        };
        let mut p = oa::Parameter::path(param.name.to_string(), format);
        p.required = true;
        parameters.push(p.into());
    }
    let body = make_body(&rr.request.body);
    Ok(oa::Operation {
        operation_id: Some(rr.operation_id().to_string()),
        parameters,
        request_body: body,
        responses: oa::Responses {
            default: None,
            responses: indexmap! {
                oa::StatusCode::Code(200) => ReferenceOr::Item(response),
            },
            extensions: Default::default(),
        },
        ..oa::Operation::default()
    })
}

pub fn create_paths(rrs: &Vec<RequestResponse>, paths: &mut oa::Paths) -> anyhow::Result<()> {
    for rr in rrs {
        let operation = create_operation(rr)?;
        let method = oa::PathMethod::from_str(rr.info.method.to_uppercase().as_str()).unwrap();
        paths.insert_operation(rr.info.path.clone(), method, operation);
    }
    Ok(())
}
