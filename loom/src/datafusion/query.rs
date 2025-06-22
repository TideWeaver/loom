//! Query engine implementation using DataFusion

use std::sync::Arc;

use datafusion::arrow::record_batch::RecordBatch;
use datafusion::prelude::*;
use snafu::prelude::*;

use crate::Loom;
use crate::error::Result;

/// DataFusion-based query engine for executing SQL queries
pub struct QueryEngine {
    ctx: SessionContext,
    loom: Arc<Loom>,
}

impl QueryEngine {
    /// Creates a new QueryEngine instance from a Loom connection
    pub async fn new(loom: Arc<Loom>) -> Result<Self> {
        let ctx = SessionContext::new();

        Ok(QueryEngine { ctx, loom })
    }

    /// Executes a SQL query and returns the results as Arrow RecordBatches
    pub async fn query_to_arrow(&self, sql: &str) -> Result<Vec<RecordBatch>> {
        let df = self.ctx.sql(sql).await.context(super::SqlParsingSnafu)?;

        let batches = df.collect().await.context(super::ExecutionSnafu)?;

        Ok(batches)
    }

    /// Executes a SQL query and returns a DataFrame for further processing
    pub async fn query_to_dataframe(&self, sql: &str) -> Result<DataFrame> {
        let df = self.ctx.sql(sql).await.context(super::SqlParsingSnafu)?;

        Ok(df)
    }

    /// Returns the underlying DataFusion SessionContext for advanced operations
    pub fn context(&self) -> &SessionContext {
        &self.ctx
    }

    /// Returns a reference to the Loom instance
    pub fn loom(&self) -> &Loom {
        &self.loom
    }

    /// Registers a PostgreSQL table in the DataFusion context
    pub async fn register_postgres_table(&self, table_name: &str) -> crate::error::Result<()> {
        // For now, we'll use a simple implementation that doesn't actually connect to PostgreSQL
        // In a full implementation, we would:
        // 1. Query PostgreSQL for the table schema
        // 2. Create a PostgresTableProvider with the schema
        // 3. Register it with DataFusion

        // This is a placeholder that creates an empty table
        use datafusion::arrow::datatypes::{DataType, Field, Schema};

        let schema = Arc::new(Schema::new(vec![Field::new(
            "placeholder",
            DataType::Utf8,
            true,
        )]));

        let provider = Arc::new(super::provider::PostgresTableProvider::new(
            table_name.to_string(),
            schema,
        ));

        self.ctx
            .register_table(table_name, provider)
            .context(super::TableRegistrationSnafu)?;

        Ok(())
    }
}

impl Loom {
    /// Creates a new QueryEngine instance for DataFusion operations
    pub async fn query_engine(self: Arc<Self>) -> Result<QueryEngine> {
        QueryEngine::new(self).await
    }

    /// Convenience method to execute a query and return Arrow results
    pub async fn query_to_arrow(&self, sql: &str) -> Result<Vec<RecordBatch>> {
        let engine = Arc::new(self.clone()).query_engine().await?;
        engine.query_to_arrow(sql).await
    }
}

// We need to make Loom cloneable for Arc usage
impl Clone for Loom {
    fn clone(&self) -> Self {
        Loom {
            pool: self.pool.clone(),
        }
    }
}
