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
    /// Export table to Parquet format
    Export(ExportArgs),
    /// Import data from Parquet snapshot
    Import(ImportArgs),
    /// List available snapshots
    ListSnapshots(ListSnapshotsArgs),
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
    /// Output format (csv, table)
    #[clap(long, default_value = "table")]
    format: String,
    /// Output file path (optional, defaults to stdout)
    #[clap(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
#[clap(about = "Export table to Parquet format")]
struct ExportArgs {
    /// Database name
    database: String,
    /// Table name (format: schema.table or just table)
    table: String,
}

#[derive(Debug, clap::Args)]
#[clap(about = "Import data from Parquet snapshot")]
struct ImportArgs {
    /// Database name
    database: String,
    /// Table name
    table: String,
    /// Snapshot timestamp (optional, defaults to latest)
    #[clap(long)]
    snapshot: Option<String>,
    /// Truncate table before import
    #[clap(long)]
    truncate: bool,
    /// Create table if not exists
    #[clap(long, default_value_t = true)]
    create_if_not_exists: bool,
}

#[derive(Debug, clap::Args)]
#[clap(about = "List available snapshots")]
struct ListSnapshotsArgs {
    /// Database name
    database: String,
    /// Table name
    table: String,
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
        Commands::Export(export_args) => export_table(loom, export_args).await,
        Commands::Import(import_args) => import_snapshot(loom, import_args).await,
        Commands::ListSnapshots(list_args) => list_snapshots(loom, list_args).await,
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

async fn export_table(loom: Loom, args: ExportArgs) -> Result<(), Error> {
    // Parse schema.table format
    let (schema, table) = if args.table.contains('.') {
        let parts: Vec<&str> = args.table.split('.').collect();
        (Some(parts[0]), parts[1])
    } else {
        (None, args.table.as_str())
    };

    println!(
        "Exporting table {}.{} from database {}...",
        schema.unwrap_or("public"),
        table,
        args.database
    );

    let metadata = loom
        .export_table(&args.database, schema, table)
        .await
        .context(LoomSnafu)?;

    println!("Export completed successfully!");
    println!("Snapshot time: {}", metadata.snapshot_time);
    println!("Row count: {}", metadata.table_info.row_count);
    println!("Snapshot path: {}", metadata.get_snapshot_path(""));

    Ok(())
}

async fn import_snapshot(loom: Loom, args: ImportArgs) -> Result<(), Error> {
    // First, list available snapshots if no specific snapshot provided
    let snapshot_path = if let Some(snapshot) = args.snapshot {
        format!("{}_{}/{}", args.database, args.table, snapshot)
    } else {
        // Get the latest snapshot
        let snapshots = loom
            .list_snapshots(&args.database, &args.table)
            .await
            .context(LoomSnafu)?;

        if snapshots.is_empty() {
            println!(
                "No snapshots found for table {}.{}",
                args.database, args.table
            );
            return Ok(());
        }

        format!("{}_{}/{}", args.database, args.table, snapshots[0])
    };

    println!("Importing from snapshot: {}", snapshot_path);

    let options = loom::parquet::ImportOptions {
        truncate_before_import: args.truncate,
        create_table_if_not_exists: args.create_if_not_exists,
        ..Default::default()
    };

    let row_count = loom
        .import_snapshot(&snapshot_path, options)
        .await
        .context(LoomSnafu)?;

    println!("Import completed successfully!");
    println!("Rows imported: {}", row_count);

    Ok(())
}

async fn list_snapshots(loom: Loom, args: ListSnapshotsArgs) -> Result<(), Error> {
    println!(
        "Listing snapshots for table {}.{}",
        args.database, args.table
    );

    let snapshots = loom
        .list_snapshots(&args.database, &args.table)
        .await
        .context(LoomSnafu)?;

    if snapshots.is_empty() {
        println!("No snapshots found.");
    } else {
        println!("Available snapshots:");
        for (i, snapshot) in snapshots.iter().enumerate() {
            println!("  {}. {}", i + 1, snapshot);
        }
    }

    Ok(())
}
