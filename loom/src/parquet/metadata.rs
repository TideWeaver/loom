//! Metadata structures for Parquet snapshots

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMetadata {
    pub version: String,
    pub snapshot_time: DateTime<Utc>,
    pub source: SourceInfo,
    pub table_info: TableInfo,
    pub snapshot_info: SnapshotInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    #[serde(rename = "type")]
    pub db_type: DatabaseType,
    pub database: String,
    pub schema: Option<String>,
    pub table: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub row_count: i64,
    pub size_bytes: i64,
    pub columns: Vec<ColumnMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMetadata {
    pub name: String,
    pub data_type: String,
    pub is_primary_key: bool,
    pub is_nullable: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub extraction_method: String,
    pub compression: String,
    pub row_group_size: usize,
}

impl TableMetadata {
    pub fn new(
        db_type: DatabaseType,
        database: String,
        schema: Option<String>,
        table: String,
    ) -> Self {
        Self {
            version: "1.0".to_string(),
            snapshot_time: Utc::now(),
            source: SourceInfo {
                db_type,
                database,
                schema,
                table,
            },
            table_info: TableInfo {
                row_count: 0,
                size_bytes: 0,
                columns: vec![],
            },
            snapshot_info: SnapshotInfo {
                extraction_method: "COPY".to_string(),
                compression: "snappy".to_string(),
                row_group_size: 100_000,
            },
        }
    }

    pub fn get_snapshot_path(&self, base_path: &str) -> String {
        let table_prefix = match &self.source.schema {
            Some(schema) => format!("{}_{}", schema, self.source.table),
            None => self.source.table.clone(),
        };

        format!(
            "{}/{}_{}/{}",
            base_path,
            self.source.database,
            table_prefix,
            self.snapshot_time.format("%Y-%m-%d_%H-%M-%S")
        )
    }
}
