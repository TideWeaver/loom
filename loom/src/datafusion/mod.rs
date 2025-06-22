//! DataFusion integration module for Loom
//!
//! This module provides query engine capabilities using Apache Arrow DataFusion.

pub mod provider;
pub mod query;

pub use query::QueryEngine;

use crate::error;
use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum DataFusionError {
    #[snafu(display("DataFusion execution error: {}", source))]
    ExecutionError {
        source: datafusion::error::DataFusionError,
    },

    #[snafu(display("Failed to register table: {}", source))]
    TableRegistration {
        source: datafusion::error::DataFusionError,
    },

    #[snafu(display("SQL parsing error: {}", source))]
    SqlParsing {
        source: datafusion::error::DataFusionError,
    },

    #[snafu(display("PostgreSQL error: {}", source))]
    PostgresError { source: sqlx::Error },

    #[snafu(display("Arrow conversion error: {}", message))]
    ArrowConversion { message: String },
}

impl From<DataFusionError> for error::Error {
    fn from(err: DataFusionError) -> Self {
        error::Error::DataFusion {
            source: Box::new(err),
        }
    }
}
