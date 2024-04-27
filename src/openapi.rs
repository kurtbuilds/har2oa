pub mod operation;
mod parameter;
pub(crate) mod response;
mod schema;
mod str_munge;

use crate::http::{singular, Request, RequestResponse};
use anyhow::anyhow;
use anyhow::Result;
use convert_case::{Case, Casing};
use indexmap::indexmap;
use itertools::Itertools;
use once_cell::sync::Lazy;
use openapiv3 as oa;
use openapiv3::{RefOr, ReferenceOr, Type};
use regex::Regex;
use serde_json::{Map, Value};
use std::cell::OnceCell;
use std::fmt::Formatter;
use std::ops::DerefMut;
use std::str::FromStr;
use tracing::{info, warn};

/// Takes a name and returns the singular version of it
/// e.g. Vendors -> Vendor
/// e.g. VendorsResponse -> Vendor
pub fn extract_object_name(mut name: &str) -> Option<String> {
    if name.ends_with("Response") {
        name = &name[..name.len() - 8];
    }
    if name.ends_with("List") {
        name = &name[..name.len() - 4]
    }
    if name.ends_with("list") {
        name = &name[..name.len() - 4]
    }
    if name.ends_with("ies") {
        Some(format!("{}y", &name[..name.len() - 3]))
    } else if name.ends_with("s") && !name.ends_with("ss") {
        Some(name[..name.len() - 1].to_string())
    } else {
        Some(name.to_string())
    }
}

pub fn schema_name(name: &str) -> String {
    name.to_case(Case::Pascal)
}

fn infer_schema_name(key: &str, rr: &RequestResponse) -> String {
    if key.to_lowercase() == "list" {
        rr.object_name().to_string()
    } else {
        let singular = extract_object_name(&key).unwrap();
        let singular = schema_name(&singular);
        singular
    }
}

/**
getClient

GetClientResponse {
requestid: string
Data: Response
}
 */

fn use_reference(schema: &oa::Schema) -> bool {
    match &schema.kind {
        oa::SchemaKind::Type(oa::Type::Object(o)) => !o.properties.is_empty(),
        // oa::SchemaKind::Type(oa::Type::Array(a)) => {
        //     a.items.is_some()
        // }
        _ => false,
    }
}

fn is_primitive(schema: &oa::Schema) -> bool {
    match &schema.kind {
        oa::SchemaKind::Type(oa::Type::Object(o)) => o.properties.is_empty(),
        oa::SchemaKind::Type(oa::Type::Array(a)) => a.items.is_none(),
        _ => true,
    }
}

struct Lu<T>(T);

impl std::fmt::Debug for Lu<oa::ReferenceOr<oa::Schema>> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use openapiv3::SchemaKind;
        use openapiv3::Type;
        match &self.0 {
            oa::ReferenceOr::Reference { reference } => {
                write!(f, "Reference({})", reference)
            }
            oa::ReferenceOr::Item(schema) => match &schema.kind {
                SchemaKind::Type(Type::Object(o)) => {
                    let prop_keys = o.properties.keys().collect::<Vec<_>>();
                    write!(f, "Object{{{:?}}}", prop_keys)
                }
                SchemaKind::Type(Type::String(a)) => {
                    write!(f, "String")
                }
                _ => write!(f, "Item({:?})", schema),
            },
        }
    }
}

fn create_schema(
    components: &mut oa::Components,
    value: &Value,
    object_name: Option<&str>,
    rr: &RequestResponse,
) -> Result<oa::Schema> {
    let s = match value {
        Value::Null => oa::Schema::new_object(),
        Value::Bool(_) => oa::Schema::new_bool(),
        Value::Number(n) => {
            let mut s = if n.is_f64() {
                oa::Schema::new_number()
            } else {
                oa::Schema::new_integer()
            };
            if let Some(object_name) = object_name {
                static DATE_PROPERTY_NAME: Lazy<Regex> =
                    Lazy::new(|| Regex::new(r"(?:\b|_)date(?:\b|_)").unwrap());
                if DATE_PROPERTY_NAME.is_match(object_name)
                    || object_name == "delivered"
                    || object_name == "lastmodifieddate"
                    || object_name == "approved"
                    || object_name == "createddate"
                {
                    s.data
                        .extensions
                        .insert("x-format".to_string(), Value::from("date"));
                }
                if ["client", "order_c", "invoice"].contains(&object_name) {
                    s.data
                        .extensions
                        .insert("x-null-as-zero".to_string(), Value::from(true));
                }
            }
            s
        }
        Value::String(value) => {
            let mut s = oa::Schema::new_string();
            if let Some(object_name) = object_name {
                if object_name == "phone" {
                    s = s.with_format("phone");
                } else if object_name == "email" {
                    s = s.with_format("email");
                } else if object_name.contains("item")
                    || object_name.contains("name")
                    || object_name.contains("id")
                    || object_name.contains("zip")
                    || object_name.contains("postal")
                    || object_name == "order_vendor_order"
                {
                    // We just want a Schema::new_string for these situations, so do nothing.
                } else if value.parse::<f64>().is_ok() {
                    s = s.with_format("decimal");
                }
            }
            s
        }
        Value::Array(inner) => {
            // println!("Array: {}", object_name);
            // let object_name = rr.object_name();
            // First add the inner schema to the components
            let inner = if inner.len() == 0 {
                oa::Schema::new_object()
            } else {
                create_schema(components, &inner[0], object_name, rr).unwrap()
            };
            if is_primitive(&inner) {
                oa::Schema::new_array(inner)
            } else {
                let object_name = object_name
                    .unwrap_or_else(|| rr.object_name())
                    .to_case(Case::Pascal);
                let map = components.schemas.deref_mut();
                if let Some(e) = map.insert(object_name.clone(), inner.into()) {
                    warn!(name=%object_name, existing=?Lu(e), "Schema already exists. TODO consider checking schemas for equality, panicking if not equal.");
                } else {
                    info!(name=%object_name, url=rr.request.url.as_str(), "Added schema");
                }
                // Then return an array, which references the inner schema
                oa::Schema::new_array(RefOr::schema_ref(&object_name))
            }
        }
        Value::Object(map) => {
            let mut s = oa::Schema::new_object();
            for (key, value) in map {
                let schema_name = if value.is_array() && key.to_lowercase() == "list" {
                    Some(rr.object_name().to_string())
                } else {
                    Some(singular(key))
                };
                let schema_name = schema_name.as_ref().map(|s| s.as_str());
                let Ok(schema) = create_schema(components, value, schema_name, rr) else {
                    continue;
                };
                s.set_required(key, true);
                if use_reference(&schema) {
                    let schema_name = schema_name
                        .unwrap_or_else(|| rr.object_name())
                        .to_case(Case::Pascal);
                    let map = components.schemas.deref_mut();
                    if let Some(o) = map.insert(schema_name.clone(), schema.into()) {
                        warn!(name=%schema_name, existing=?Lu(o), "Schema already exists");
                    } else {
                        info!(name=%schema_name, url=rr.request.url.as_str(), "Added schema");
                    }
                    s.add_property(key, ReferenceOr::schema_ref(&schema_name));
                } else {
                    s.add_property(key, schema);
                }
            }
            s
        }
    };
    Ok(s)
}

// (Value, &mut Components) -> oa::Schema
fn add_response_schemas(components: &mut oa::Components, rr: &RequestResponse) -> Result<()> {
    let response_data = &rr.response.data;
    let schema = create_schema(components, &response_data, None, rr)?;
    let object_name = rr.response_object_name();
    let map = components.schemas.deref_mut();
    map.insert(object_name.to_string(), schema.into())
        .ok_or_else(|| panic!("Schema already exists: {}", rr.response_object_name()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::read_har;
    use crate::openapi::parameter::infer_parameter_schema;
    use crate::openapi::response::create_schema_for_responses;
    use anyhow::Result;
    use openapiv3 as oa;

    #[test]
    fn test_into_rr_swtraining() -> Result<()> {
        let rr = read_har("data/app.studiodesigner.com/api/swtraining/freetrainings.har")?
            .into_iter()
            .filter(|e| {
                e.request
                    .url
                    .starts_with("https://app.studiodesigner.com/api/")
            })
            .next()
            .map(RequestResponse::from)
            .unwrap();
        assert_eq!(rr.operation_id(), "getSwtrainingFreetrainings");
        assert_eq!(rr.object_name(), "Freetraining");
        assert_eq!(
            rr.response_object_name(),
            "GetSwtrainingFreetrainingsResponse"
        );
        assert!(matches!(rr.response.data, Value::Array(_)));
        let mut schema = oa::OpenAPI::default();
        let rrs = vec![rr];
        create_schema_for_responses(&rrs, &mut schema.components)?;
        let schema = schema
            .schemas
            .get("GetSwtrainingFreetrainingsResponse")
            .unwrap()
            .as_item()
            .unwrap();
        assert!(matches!(
            schema.kind,
            oa::SchemaKind::Type(oa::Type::Array(_))
        ));
        Ok(())
    }

    #[test]
    fn test_infer_parameter_schema() {
        let s = infer_parameter_schema("foo", "");
        assert!(matches!(s.kind, oa::SchemaKind::Type(oa::Type::String(_))));

        let s = infer_parameter_schema("foo", "32");
        assert!(matches!(s.kind, oa::SchemaKind::Type(oa::Type::Integer(_))));

        let s = infer_parameter_schema("foo", "-32");
        assert!(matches!(s.kind, oa::SchemaKind::Type(oa::Type::Integer(_))));

        let s = infer_parameter_schema("foo", "1.0");
        assert!(matches!(s.kind, oa::SchemaKind::Type(oa::Type::Number(_))));

        let s = infer_parameter_schema("foo[]", "");
        let items = match s.kind {
            oa::SchemaKind::Type(oa::Type::Array(oa::ArrayType { items, .. })) => items,
            _ => panic!("expected array"),
        };
        let items = items.unwrap().into_item().unwrap();
        assert!(matches!(
            items.kind,
            oa::SchemaKind::Type(oa::Type::String(_))
        ));
    }

    #[test]
    fn test_nested_objects_are_references() -> Result<()> {
        let rr = read_har("data/app.studiodesigner.com/api/vendors/external.har")
            .unwrap()
            .into_iter()
            .map(RequestResponse::from)
            .collect::<Vec<_>>();

        let mut spec = oa::OpenAPI::default();

        create_schema_for_responses(&rr, &mut spec.components)?;
        let schema = spec
            .schemas
            .get("GetVendorsExternalResponse")
            .unwrap()
            .as_item()
            .unwrap();
        let props = schema.properties().unwrap();
        let s = serde_yaml::to_string(&spec.components).unwrap();
        println!("{}", s);
        assert!(spec.schemas.get("Address").is_some());
        assert!(spec.schemas.get("External").is_some());
        assert!(spec.schemas.get("Logo").is_none());
        let schema = spec.schemas.get("External").unwrap().as_item().unwrap();
        let users = schema
            .properties()
            .unwrap()
            .get("users")
            .unwrap()
            .as_item()
            .unwrap();
        let items = match &users.kind {
            oa::SchemaKind::Type(oa::Type::Array(oa::ArrayType { items, .. })) => items,
            _ => panic!("expected array"),
        };
        let users_schema_ref = items.as_ref().unwrap();
        let users_schema_ref = users_schema_ref.as_ref_str().unwrap();
        assert_eq!(users_schema_ref, "#/components/schemas/User");
        Ok(())
    }

    #[test]
    fn test_nested_array_is_not_reference() -> Result<()> {
        let rr = read_har("data/app.studiodesigner.com/api/itemlist.har")
            .unwrap()
            .into_iter()
            .map(RequestResponse::from)
            .collect::<Vec<_>>();

        let mut spec = oa::OpenAPI::default();

        create_schema_for_responses(&rr, &mut spec.components)?;

        let s = serde_yaml::to_string(&spec.components).unwrap();
        println!("{}", s);
        assert!(spec.schemas.get("List").is_none());
        let schema = spec.schemas.get("Item").unwrap().as_item().unwrap();
        let invoice = schema
            .properties()
            .unwrap()
            .get("invoice")
            .unwrap()
            .as_item()
            .unwrap();
        assert!(
            invoice
                .data
                .extensions
                .get("x-null-as-zero")
                .and_then(|c| c.as_bool())
                .unwrap_or_default(),
            "invoice is x-null-as-zero"
        );
        let client_total_balance = schema
            .properties()
            .unwrap()
            .get("client_total_balance")
            .unwrap()
            .as_item()
            .unwrap();
        assert!(
            matches!(
                &client_total_balance.kind,
                oa::SchemaKind::Type(oa::Type::String(s)) if s.format.as_str() == "decimal",
            ),
            "client_total_balance is decimal (currency) as string"
        );

        let invoice_name = schema
            .properties()
            .unwrap()
            .get("invoice_name")
            .unwrap()
            .as_item()
            .unwrap();
        assert!(
            matches!(
                &invoice_name.kind,
                oa::SchemaKind::Type(oa::Type::String(s)) if s.format.as_str() == "",
            ),
            "invoice_name is string (not format: decimal/currency)"
        );

        Ok(())
    }

    #[test]
    fn test_login() -> Result<()> {
        let rr = read_har("data/app.studiodesigner.com/api/login.har")
            .unwrap()
            .into_iter()
            .map(RequestResponse::from)
            .collect::<Vec<_>>();
        let mut spec = oa::OpenAPI::default();

        create_schema_for_responses(&rr, &mut spec.components)?;

        let s = serde_yaml::to_string(&spec.components).unwrap();
        println!("{}", s);
        let res = spec
            .schemas
            .get("PostLoginResponse")
            .unwrap()
            .as_item()
            .unwrap();
        let permissions = res
            .properties()
            .unwrap()
            .get("permissions")
            .unwrap()
            .as_item()
            .unwrap();
        let oa::SchemaKind::Type(Type::Array(oa::ArrayType { items, .. })) = &permissions.kind
        else {
            panic!("expected array");
        };
        let items = *items.clone().unwrap();
        let items = items
            .into_item()
            .expect("expected item, but array item is a ref");
        assert!(matches!(
            &items.kind,
            oa::SchemaKind::Type(oa::Type::String(oa::StringType { .. }))
        ));
        Ok(())
    }
}
