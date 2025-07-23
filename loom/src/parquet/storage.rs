//! Storage backend configuration

use serde::{Deserialize, Serialize};
use snafu::prelude::*;
use std::path::Path;

#[derive(Debug, Snafu)]
pub enum StorageError {
    #[snafu(display("Failed to create directory: {}", path))]
    CreateDirectory {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("Failed to write file: {}", path))]
    WriteFile {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("Failed to read file: {}", path))]
    ReadFile {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("S3 operation failed: {}", message))]
    S3Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StorageConfig {
    Local {
        path: String,
    },
    S3 {
        bucket: String,
        prefix: String,
        region: Option<String>,
    },
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig::Local {
            path: "./data/snapshots".to_string(),
        }
    }
}

pub enum StorageBackend {
    Local,
    S3(object_store::aws::AmazonS3),
}

impl StorageConfig {
    pub async fn create_backend(&self) -> Result<StorageBackend, StorageError> {
        match self {
            StorageConfig::Local { path } => {
                // Ensure directory exists
                std::fs::create_dir_all(path).context(CreateDirectorySnafu { path })?;
                Ok(StorageBackend::Local)
            }
            StorageConfig::S3 {
                bucket,
                prefix: _,
                region,
            } => {
                let mut builder =
                    object_store::aws::AmazonS3Builder::new().with_bucket_name(bucket);

                if let Some(region) = region {
                    builder = builder.with_region(region);
                }

                let store = builder.build().map_err(|e| StorageError::S3Error {
                    message: e.to_string(),
                })?;

                Ok(StorageBackend::S3(store))
            }
        }
    }

    pub fn get_base_path(&self) -> String {
        match self {
            StorageConfig::Local { path } => path.clone(),
            StorageConfig::S3 { bucket, prefix, .. } => {
                format!("s3://{}/{}", bucket, prefix)
            }
        }
    }

    pub async fn write_bytes(&self, path: &str, data: &[u8]) -> Result<(), StorageError> {
        match self {
            StorageConfig::Local { path: base_path } => {
                let full_path = Path::new(base_path).join(path);
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent).context(CreateDirectorySnafu {
                        path: parent.display().to_string(),
                    })?;
                }
                std::fs::write(&full_path, data).context(WriteFileSnafu {
                    path: full_path.display().to_string(),
                })?;
                Ok(())
            }
            StorageConfig::S3 { .. } => {
                // TODO: Implement S3 write
                Err(StorageError::S3Error {
                    message: "S3 write not implemented yet".to_string(),
                })
            }
        }
    }

    pub async fn read_bytes(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        match self {
            StorageConfig::Local { path: base_path } => {
                let full_path = Path::new(base_path).join(path);
                std::fs::read(&full_path).context(ReadFileSnafu {
                    path: full_path.display().to_string(),
                })
            }
            StorageConfig::S3 { .. } => {
                // TODO: Implement S3 read
                Err(StorageError::S3Error {
                    message: "S3 read not implemented yet".to_string(),
                })
            }
        }
    }
}
