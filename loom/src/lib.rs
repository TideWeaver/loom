//! Loom library crate
//!
//! This crate provides the core functionality for the loom project.

pub mod datafusion;
pub mod error;
pub mod parquet;

use error::Result;
use snafu::prelude::*;

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DatabaseConfig {
    Postgres { url: String },
    Mysql { url: String },
}

#[derive(serde::Deserialize, Debug, Default)]
pub struct Config {
    pub database: DatabaseConfig,
    #[serde(default)]
    pub storage: parquet::StorageConfig,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        DatabaseConfig::Postgres { url: String::new() }
    }
}

// Database connection enum
#[derive(Debug, Clone)]
pub enum DatabasePool {
    Postgres(sqlx::PgPool),
    Mysql(sqlx::MySqlPool),
}

// Main Application
#[derive(Debug)]
pub struct Loom {
    pool: DatabasePool,
    storage_config: parquet::StorageConfig,
}

impl Loom {
    /// Get the database pool
    pub fn pool(&self) -> &DatabasePool {
        &self.pool
    }

    /// Get the database type
    pub fn db_type(&self) -> &str {
        match &self.pool {
            DatabasePool::Postgres(_) => "postgres",
            DatabasePool::Mysql(_) => "mysql",
        }
    }
    /// Creates a new Loom instance from a configuration file.
    pub async fn new_from_config(config_root: impl AsRef<std::path::Path>) -> Result<Self> {
        let mut builder = config::Config::builder();
        // try to load config file
        let config_path = config_root.as_ref().join("config");
        if config_path.with_extension("yml").exists() || config_path.with_extension("yaml").exists()
        {
            builder = builder.add_source(config::File::from(config_path));
        }
        // always try to read from environment variables
        builder = builder.add_source(config::Environment::with_prefix("LOOM"));

        let cfg = builder
            .build()
            .context(error::ConfigSnafu)?
            .try_deserialize::<Config>()
            .context(error::ConfigSnafu)?;

        let pool = match &cfg.database {
            DatabaseConfig::Postgres { url } => DatabasePool::Postgres(
                sqlx::postgres::PgPool::connect(url)
                    .await
                    .context(error::PgSnafu)?,
            ),
            DatabaseConfig::Mysql { url } => DatabasePool::Mysql(
                sqlx::mysql::MySqlPool::connect(url)
                    .await
                    .context(error::MysqlSnafu)?,
            ),
        };

        Ok(Loom {
            pool,
            storage_config: cfg.storage,
        })
    }

    /// Creates a new Loom instance directly from a database URL.
    pub async fn new_from_url(database_url: &str, db_type: &str) -> Result<Self> {
        let pool = match db_type {
            "postgres" | "postgresql" => DatabasePool::Postgres(
                sqlx::postgres::PgPool::connect(database_url)
                    .await
                    .context(error::PgSnafu)?,
            ),
            "mysql" => DatabasePool::Mysql(
                sqlx::mysql::MySqlPool::connect(database_url)
                    .await
                    .context(error::MysqlSnafu)?,
            ),
            _ => {
                return Err(error::Error::Generic {
                    message: format!("Unsupported database type: {}", db_type),
                });
            }
        };

        Ok(Loom {
            pool,
            storage_config: parquet::StorageConfig::default(),
        })
    }

    /// Creates a new Loom instance from environment variables only.
    pub async fn new_from_env() -> Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::Environment::with_prefix("LOOM"))
            .build()
            .context(error::ConfigSnafu)?
            .try_deserialize::<Config>()
            .context(error::ConfigSnafu)?;

        let pool = match &cfg.database {
            DatabaseConfig::Postgres { url } => DatabasePool::Postgres(
                sqlx::postgres::PgPool::connect(url)
                    .await
                    .context(error::PgSnafu)?,
            ),
            DatabaseConfig::Mysql { url } => DatabasePool::Mysql(
                sqlx::mysql::MySqlPool::connect(url)
                    .await
                    .context(error::MysqlSnafu)?,
            ),
        };

        Ok(Loom {
            pool,
            storage_config: cfg.storage,
        })
    }

    /// Executes a given SQL query.
    pub async fn execute_sql(&self, query: &str) -> Result<()> {
        match &self.pool {
            DatabasePool::Postgres(pool) => {
                sqlx::query(query)
                    .execute(pool)
                    .await
                    .context(error::PgSnafu)?;
            }
            DatabasePool::Mysql(pool) => {
                sqlx::query(query)
                    .execute(pool)
                    .await
                    .context(error::MysqlSnafu)?;
            }
        }
        Ok(())
    }

    /// Export a table to Parquet format
    pub async fn export_table(
        &self,
        database: &str,
        schema: Option<&str>,
        table: &str,
    ) -> Result<parquet::TableMetadata> {
        let exporter = match &self.pool {
            DatabasePool::Postgres(pool) => parquet::create_postgres_exporter(
                pool.clone(),
                parquet::ExportOptions {
                    storage_config: self.storage_config.clone(),
                    ..Default::default()
                },
            ),
            DatabasePool::Mysql(pool) => parquet::create_mysql_exporter(
                pool.clone(),
                parquet::ExportOptions {
                    storage_config: self.storage_config.clone(),
                    ..Default::default()
                },
            ),
        };

        exporter
            .export_table(database, schema, table)
            .await
            .map_err(|e| error::Error::Generic {
                message: format!("Export failed: {}", e),
            })
    }

    /// Import data from a Parquet snapshot
    pub async fn import_snapshot(
        &self,
        snapshot_path: &str,
        options: parquet::ImportOptions,
    ) -> Result<i64> {
        let importer = match &self.pool {
            DatabasePool::Postgres(pool) => {
                parquet::create_postgres_importer(pool.clone(), options)
            }
            DatabasePool::Mysql(pool) => parquet::create_mysql_importer(pool.clone(), options),
        };

        importer
            .import_snapshot(snapshot_path, &self.storage_config)
            .await
            .map_err(|e| error::Error::Generic {
                message: format!("Import failed: {}", e),
            })
    }

    /// List available snapshots for a table
    pub async fn list_snapshots(&self, database: &str, table: &str) -> Result<Vec<String>> {
        let importer = match &self.pool {
            DatabasePool::Postgres(pool) => {
                parquet::create_postgres_importer(pool.clone(), parquet::ImportOptions::default())
            }
            DatabasePool::Mysql(pool) => {
                parquet::create_mysql_importer(pool.clone(), parquet::ImportOptions::default())
            }
        };

        importer
            .list_snapshots(database, table, &self.storage_config)
            .await
            .map_err(|e| error::Error::Generic {
                message: format!("Failed to list snapshots: {}", e),
            })
    }
}
