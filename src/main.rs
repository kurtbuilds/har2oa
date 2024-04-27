#![allow(unused)]

use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::IsTerminal;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use convert_case::{Case, Casing};
use har::{Har, Spec};
use har::v1_2::Entries;
use indexmap::{IndexMap, indexmap};
use itertools::Itertools;
use openapiv3 as oa;
use openapiv3::{AdditionalProperties, Components, Info, NumberType, ObjectType, OpenAPI, Operation, PathItem, Paths, ReferenceOr, Responses, Schema, SchemaData, SchemaKind, Server, Type};
use serde_json::{Map, Value};

use command::*;

mod http;
mod openapi;
mod command;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Command,

    #[clap(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate an OpenAPI spec from a HAR file
    Generate(Generate),
    // Merge(Merge),
    /// Merge multiple Har logs into one
    Merge(MergeHar),
    /// Filter
    Filter(Filter),
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .without_time()
        .with_ansi(std::io::stdin().is_terminal())
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Generate(g) => g.run(),
        Command::Merge(m) => m.run(),
        Command::Filter(f) => f.run(),
        Command::Merge(m) => m.run(),
    }
}