use config::ConfigError;
use snafu::prelude::*;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("IO error: {source}"))]
    Io { source: std::io::Error },

    #[snafu(display("PG error: {source}"))]
    Pg { source: sqlx::Error },

    #[snafu(display("MySQL error: {source}"))]
    Mysql { source: sqlx::Error },

    #[snafu(display("Config error: {source}"))]
    Config { source: ConfigError },

    #[snafu(display("Generic error: {message}"))]
    Generic { message: String },

    #[snafu(display("DataFusion error: {source}"))]
    DataFusion {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
