use openapiv3 as oa;
use std::{env, fs};

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut schema: oa::OpenAPI =
        serde_yaml::from_reader(fs::File::open(&args[1]).unwrap()).unwrap();

    schema.paths.sort_keys();
    schema.schemas.sort_keys();

    let s = serde_yaml::to_string(&schema).unwrap();
    fs::write(&args[1], &s).unwrap();
}
