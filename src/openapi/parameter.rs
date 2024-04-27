use anyhow::Result;
use openapiv3 as oa;

/// Examine the key (e.g. "id[]") and attempt parses (e.g. int, float) on the value to infer
/// an oa::Schema for the parameter.
pub fn infer_parameter_schema(key: &str, value: &str) -> oa::Schema {
    if key.ends_with("[]") {
        let key = &key[..key.len() - 2];
        let inner_schema = infer_parameter_schema(key, value);
        return oa::Schema::new_array(inner_schema);
    }
    let schema = if value.parse::<i32>().is_ok() {
        oa::Schema::new_integer()
    } else if value.parse::<f32>().is_ok() {
        oa::Schema::new_number()
    } else if value == "true" || value == "false" {
        oa::Schema::new_bool()
    } else {
        oa::Schema::new_string()
    };
    schema
}

fn sanitize_parameter_key(key: &str) -> String {
    key.replace("[]", "")
}

/// query parameters are only strings, so we have to infer the type from the value
pub fn create_parameter(key: &str, value: &str) -> Result<oa::Parameter> {
    let schema = infer_parameter_schema(key, value);
    let name = sanitize_parameter_key(key);
    let p = oa::Parameter::query(name, schema);
    Ok(p)
}
