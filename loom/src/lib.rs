//! Loom library crate
//!
//! This crate provides the core functionality for the loom project.

pub mod datafusion;
pub mod error;

use error::Result;
use snafu::prelude::*;

#[derive(serde::Deserialize, Debug, Default)]
pub struct Config {
    pub database_url: String,
}

// Main Application
#[derive(Debug)]
pub struct Loom {
    pool: sqlx::PgPool,
}

impl Loom {
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

        let pool = sqlx::postgres::PgPool::connect(&cfg.database_url)
            .await
            .context(error::PgSnafu)?;

        Ok(Loom { pool })
    }

    /// Creates a new Loom instance directly from a database URL.
    pub async fn new_from_url(database_url: &str) -> Result<Self> {
        let pool = sqlx::postgres::PgPool::connect(database_url)
            .await
            .context(error::PgSnafu)?;

        Ok(Loom { pool })
    }

    /// Creates a new Loom instance from environment variables only.
    pub async fn new_from_env() -> Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::Environment::with_prefix("LOOM"))
            .build()
            .context(error::ConfigSnafu)?
            .try_deserialize::<Config>()
            .context(error::ConfigSnafu)?;

        let pool = sqlx::postgres::PgPool::connect(&cfg.database_url)
            .await
            .context(error::PgSnafu)?;

        Ok(Loom { pool })
    }

    /// Executes a given SQL query.
    pub async fn execute_sql(&self, query: &str) -> Result<()> {
        sqlx::query(query)
            .execute(&self.pool)
            .await
            .context(error::PgSnafu)?;
        Ok(())
    }
}
