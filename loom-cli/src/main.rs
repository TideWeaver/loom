use std::path::PathBuf;
use std::sync::Arc;

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
    /// Query data using DataFusion and return as Arrow format
    Query(QueryArgs),
}

#[derive(Debug, clap::Args)]
#[clap(about = "Execute a SQL query")]
struct ExecArgs {
    /// SQL query to execute
    query: String,
}

#[derive(Debug, clap::Args)]
#[clap(about = "Query data using DataFusion")]
struct QueryArgs {
    /// SQL query to execute
    sql: String,
    /// Output format (json, csv, table)
    #[clap(long, default_value = "table")]
    format: String,
    /// Output file path (optional, defaults to stdout)
    #[clap(long)]
    output: Option<PathBuf>,
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
        Commands::Query(query_args) => query_data(loom, query_args).await,
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

async fn query_data(loom: Loom, args: QueryArgs) -> Result<(), Error> {
    println!("Executing DataFusion query: {}", args.sql);

    let engine = Arc::new(loom).query_engine().await.context(LoomSnafu)?;
    let batches = engine.query_to_arrow(&args.sql).await.context(LoomSnafu)?;

    match args.format.as_str() {
        "table" => {
            // Pretty print as table
            use datafusion::arrow::util::pretty::pretty_format_batches;
            let formatted = pretty_format_batches(&batches).map_err(|e| Error::Loom {
                source: loom::error::Error::Generic {
                    message: e.to_string(),
                },
            })?;
            println!("{}", formatted);
        }
        "json" => {
            // Output as JSON
            use datafusion::arrow::json::writer::record_batches_to_json_rows;
            use datafusion::arrow::record_batch::RecordBatch;
            let batch_refs: Vec<&RecordBatch> = batches.iter().collect();
            let json_rows = record_batches_to_json_rows(&batch_refs).map_err(|e| Error::Loom {
                source: loom::error::Error::Generic {
                    message: e.to_string(),
                },
            })?;
            let json_string =
                serde_json::to_string_pretty(&json_rows).map_err(|e| Error::Loom {
                    source: loom::error::Error::Generic {
                        message: e.to_string(),
                    },
                })?;

            if let Some(output_path) = args.output {
                std::fs::write(output_path, json_string).map_err(|e| Error::Loom {
                    source: loom::error::Error::Generic {
                        message: e.to_string(),
                    },
                })?;
                println!("Results written to file");
            } else {
                println!("{}", json_string);
            }
        }
        "csv" => {
            // Output as CSV
            use datafusion::arrow::csv::WriterBuilder;

            let mut buffer = Vec::new();
            {
                let mut writer = WriterBuilder::new().build(&mut buffer);
                for batch in &batches {
                    writer.write(batch).map_err(|e| Error::Loom {
                        source: loom::error::Error::Generic {
                            message: e.to_string(),
                        },
                    })?;
                }
            }

            let csv_string = String::from_utf8(buffer).map_err(|e| Error::Loom {
                source: loom::error::Error::Generic {
                    message: e.to_string(),
                },
            })?;

            if let Some(output_path) = args.output {
                std::fs::write(output_path, csv_string).map_err(|e| Error::Loom {
                    source: loom::error::Error::Generic {
                        message: e.to_string(),
                    },
                })?;
                println!("Results written to file");
            } else {
                print!("{}", csv_string);
            }
        }
        _ => {
            return Err(Error::Loom {
                source: loom::error::Error::Generic {
                    message: format!("Unsupported format: {}", args.format),
                },
            });
        }
    }

    Ok(())
}
