use anyhow::Result;
use clap::Args;
use openapiv3::OpenAPI;
use std::fs::File;

#[derive(Debug, Args)]
pub struct Merge {
    #[arg(num_args = 1..)]
    files: Vec<String>,

    #[arg(short, long)]
    output: Option<String>,
}

impl Merge {
    pub fn run(self) -> Result<()> {
        let mut it = self.files.into_iter();
        let first = it.next().unwrap();
        let mut spec = serde_yaml::from_reader::<_, OpenAPI>(File::open(&first)?)?;

        eprintln!("{}: Read file to spec.", first);
        for filepath in it {
            let update = serde_yaml::from_reader::<_, OpenAPI>(File::open(&filepath)?)?;
            spec = spec.merge(update).map_err(|e| anyhow::anyhow!(e))?;
            eprintln!("{}: Added file to spec.", filepath);
        }
        let output = serde_yaml::to_string(&spec).map_err(|e| anyhow::anyhow!(e))?;
        if let Some(path) = self.output {
            std::fs::write(&path, &output)?;
            eprintln!("{}: Wrote file.", path);
        } else {
            println!("{}", output);
        }
        Ok(())
    }
}
