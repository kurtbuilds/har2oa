use crate::http::{ParameterType, RequestResponse};
use crate::openapi;
use crate::openapi::parameter;
use indexmap::indexmap;
use itertools::Itertools;
use openapiv3 as oa;
use openapiv3::{RefOr, ReferenceOr};
use std::str::FromStr;

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
    Ok(oa::Operation {
        operation_id: Some(rr.operation_id().to_string()),
        parameters,
        request_body: None,
        responses: oa::Responses {
            default: None,
            responses: indexmap! {
                oa::StatusCode::Code(200) => oa::ReferenceOr::Item(response),
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
