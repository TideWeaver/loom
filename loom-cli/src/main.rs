use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[clap(
    name = "loom",
    author,
    about = "A command line interface for Loom",
    version = "0.0.1"
)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    List(ListArgs),
}

#[derive(Debug, clap::Args)]
#[clap(author, about = "list resources", version = "0.0.1")]
struct ListArgs {}

use snafu::prelude::*;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("ID may not be less than 10, but it was {id}"))]
    InvalidId { id: u16 },
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    match args.command {
        Commands::List(args) => list(args),
    }
}

fn list(_args: ListArgs) -> Result<(), Error> {
    println!("list");
    Ok(())
}
