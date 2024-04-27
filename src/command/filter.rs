use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;
use clap::Args;
use anyhow::Result;
use har::Spec;
use itertools::Itertools;
use url::Url;

#[derive(Debug, Args)]
pub struct Filter {
    pub har_file: String,
    /// only take *paths* that start with this arg.
    /// So pass /api if you want only paths that start with /api
    pub only_path: String,

    /// Exclude paths
    #[arg(short, long)]
    pub exclude: Option<Vec<String>>,

    #[arg(short, long)]
    pub output: Option<String>,
}

impl Filter {
    pub fn run(self) -> Result<()> {
        let mut har = har::from_reader(File::open(&self.har_file)?)?;
        let mut entries = match har.log {
            Spec::V1_2(har::v1_2::Log { ref mut entries, .. }) => std::mem::take(entries),
            Spec::V1_3(har::v1_3::Log { .. }) => unimplemented!(),
        };
        let excludes = self.exclude.unwrap_or_default();
        entries.retain(|e| {
            let url = Url::from_str(&e.request.url).unwrap();
            if excludes.iter().any(|e| url.path().starts_with(e)) {
                return false;
            }
            if url.path().starts_with(&self.only_path) {
                return true;
            }
            false
        });
        let entries = entries.into_iter()
            .unique_by(|e| Url::from_str(&e.request.url).unwrap().path().to_string())
            .collect::<Vec<_>>();
        match har.log {
            Spec::V1_2(ref mut log) => log.entries = entries,
            Spec::V1_3(_) => unimplemented!(),
        }
        let s = serde_json::to_string_pretty(&har)?;
        if let Some(output) = self.output {
            fs::write(&output, &s)?;
            eprintln!("{}: Wrote file.", output);
        } else {
            println!("{}", s);
        }
        Ok(())
    }
}