use std::fs::File;
use anyhow::Result;
use clap::Args;
use har::v1_2::Entries;
use openapiv3::OpenAPI;

#[derive(Debug, Args)]
pub struct MergeHar {
    #[arg(num_args = 1..)]
    files: Vec<String>,

    #[arg(short, long)]
    output: Option<String>,
}

impl MergeHar {
    pub fn run(self) -> Result<()> {
        let mut iter = self.files.into_iter();
        let first = iter.next().unwrap();
        let mut har = har::from_path(&first).unwrap();

        let ::har::Spec::V1_2(har::v1_2::Log { entries, .. }) = &mut har.log else {
            unimplemented!();
        };

        for f in iter {
            let next = ::har::from_path(&f).unwrap();
            let ::har::Spec::V1_2(har::v1_2::Log { entries: next_entries, .. }) = next.log else {
                unimplemented!();
            };
            entries.extend(next_entries);
        }

        let har = ::har::to_json(&har).unwrap();
        if let Some(path) = self.output {
            std::fs::write(&path, &har)?;
            eprintln!("{}: Wrote file.", path);
        } else {
            println!("{}", har);
        }
        Ok(())
    }
}