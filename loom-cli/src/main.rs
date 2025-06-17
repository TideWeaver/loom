use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;
use loom::Loom;
use snafu::prelude::*;

#[derive(Debug, Parser)]
#[clap(
    name = "loom",
    author,
    about = "A tool to interact with database",
    version = "0.0.1"
)]
struct Args {
    /// Path to the configuration file
    #[clap(long, short, default_value = "./config.yml")]
    config: PathBuf,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Execute a SQL query
    Exec(ExecArgs),
    /// Test database connectivity
    Ping,
}

#[derive(Debug, clap::Args)]
#[clap(about = "Execute a SQL query")]
struct ExecArgs {
    /// SQL query to execute
    query: String,
}

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("Loom error: {source}"))]
    Loom { source: loom::error::Error },
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    // Get the config directory (parent of config file)
    let config_dir = args.config.parent().unwrap_or(std::path::Path::new("."));
    let loom = Loom::new_from_config(config_dir).await.context(LoomSnafu)?;
    match args.command {
        Commands::Exec(exec_args) => execute_query(loom, exec_args).await,
        Commands::Ping => ping_database(loom).await,
    }
}

async fn execute_query(loom: Loom, args: ExecArgs) -> Result<(), Error> {
    println!("Executing query: {}", args.query);
    loom.execute_sql(&args.query).await.context(LoomSnafu)?;
    println!("Query executed successfully!");
    Ok(())
}

async fn ping_database(loom: Loom) -> Result<(), Error> {
    println!("Testing database connectivity...");
    loom.execute_sql("SELECT 1").await.context(LoomSnafu)?;
    println!("Database connection successful!");
    Ok(())
}
