

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[clap(
    name = "loom",
    author,
    about = "A tool to interact with database",
    version = "0.0.1"
)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Execution(ExecutionArgs),
}


#[derive(Debug, clap::Args)]
#[clap(author, about = "execute a query", version = "0.0.1")]
struct ExecutionArgs {}

use loom::Loom;
use snafu::prelude::*;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("ID may not be less than 10, but it was {id}"))]
    InvalidId { id: u16 },
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let m = Loom::new_from_config()

    match args.command {
        Commands::Execution(args) => list(args),
    }
}

fn list(_args: ListArgs) -> Result<(), Error> {
    println!("list");
    Ok(())
}
