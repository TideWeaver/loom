//! Loom library crate
//!
//! This crate provides the core functionality for the loom project.

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
        let cfg = config::Config::builder()
            .add_source(config::File::from(config_root.as_ref().join("config")))
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
