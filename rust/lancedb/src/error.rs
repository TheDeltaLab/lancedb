// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

use std::sync::PoisonError;

use arrow_schema::ArrowError;
use datafusion_common::DataFusionError;
use snafu::Snafu;

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Invalid table name (\"{name}\"): {reason}"))]
    InvalidTableName { name: String, reason: String },
    #[snafu(display("Invalid input, {message}"))]
    InvalidInput { message: String },
    #[snafu(display("Table '{name}' was not found"))]
    TableNotFound { name: String, source: BoxError },
    #[snafu(display("Database '{name}' was not found"))]
    DatabaseNotFound { name: String },
    #[snafu(display("Database '{name}' already exists."))]
    DatabaseAlreadyExists { name: String },
    #[snafu(display("Index '{name}' was not found"))]
    IndexNotFound { name: String },
    #[snafu(display("Embedding function '{name}' was not found. : {reason}"))]
    EmbeddingFunctionNotFound { name: String, reason: String },

    #[snafu(display("Table '{name}' already exists"))]
    TableAlreadyExists { name: String },
    #[snafu(display("Unable to created lance dataset at {path}: {source}"))]
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    #[snafu(display("Schema Error: {message}"))]
    Schema { message: String },
    #[snafu(display("Runtime error: {message}"))]
    Runtime { message: String },
    #[snafu(display("Timeout error: {message}"))]
    Timeout { message: String },

    // 3rd party / external errors
    #[snafu(display("object_store error: {source}"))]
    ObjectStore { source: object_store::Error },
    #[snafu(display("lance error: {source}"))]
    Lance { source: lance::Error },
    #[snafu(display("Arrow error: {source}"))]
    Arrow { source: ArrowError },
    #[snafu(display("LanceDBError: not supported: {message}"))]
    NotSupported { message: String },
    /// External error pass through from user code.
    #[snafu(transparent)]
    External { source: BoxError },
    #[snafu(whatever, display("{message}"))]
    Other {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error + Send + Sync>, Some)))]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<ArrowError> for Error {
    fn from(source: ArrowError) -> Self {
        match source {
            ArrowError::ExternalError(source) => Self::from_box_error(source),
            _ => Self::Arrow { source },
        }
    }
}

impl From<DataFusionError> for Error {
    fn from(source: DataFusionError) -> Self {
        match source {
            DataFusionError::ArrowError(source, _) => (*source).into(),
            DataFusionError::External(source) => Self::from_box_error(source),
            other => Self::External {
                source: Box::new(other),
            },
        }
    }
}

impl From<lance::Error> for Error {
    fn from(source: lance::Error) -> Self {
        // Try to unwrap external errors that were wrapped by lance
        match source {
            lance::Error::Wrapped { error, .. } => Self::from_box_error(error),
            lance::Error::External { source } => Self::from_box_error(source),
            _ => Self::Lance { source },
        }
    }
}

impl Error {
    fn from_box_error(mut source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        source = match source.downcast::<Self>() {
            Ok(e) => match *e {
                Self::External { source } => return Self::from_box_error(source),
                other => return other,
            },
            Err(source) => source,
        };

        source = match source.downcast::<lance::Error>() {
            Ok(e) => match *e {
                lance::Error::Wrapped { error, .. } => return Self::from_box_error(error),
                other => return other.into(),
            },
            Err(source) => source,
        };

        source = match source.downcast::<ArrowError>() {
            Ok(e) => match *e {
                ArrowError::ExternalError(source) => return Self::from_box_error(source),
                other => return other.into(),
            },
            Err(source) => source,
        };

        source = match source.downcast::<DataFusionError>() {
            Ok(e) => match *e {
                DataFusionError::ArrowError(source, _) => return (*source).into(),
                DataFusionError::External(source) => return Self::from_box_error(source),
                other => return other.into(),
            },
            Err(source) => source,
        };

        Self::External { source }
    }
}

impl From<object_store::Error> for Error {
    fn from(source: object_store::Error) -> Self {
        Self::ObjectStore { source }
    }
}

impl From<object_store::path::Error> for Error {
    fn from(source: object_store::path::Error) -> Self {
        Self::ObjectStore {
            source: object_store::Error::InvalidPath { source },
        }
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(e: PoisonError<T>) -> Self {
        Self::Runtime {
            message: e.to_string(),
        }
    }
}

#[cfg(feature = "polars")]
impl From<polars::prelude::PolarsError> for Error {
    fn from(source: polars::prelude::PolarsError) -> Self {
        Self::Other {
            message: "Error in Polars DataFrame integration.".to_string(),
            source: Some(Box::new(source)),
        }
    }
}

#[cfg(feature = "sentence-transformers")]
impl From<hf_hub::api::sync::ApiError> for Error {
    fn from(source: hf_hub::api::sync::ApiError) -> Self {
        Self::Other {
            message: "Error in Sentence Transformers integration.".to_string(),
            source: Some(Box::new(source)),
        }
    }
}
#[cfg(feature = "sentence-transformers")]
impl From<candle_core::Error> for Error {
    fn from(source: candle_core::Error) -> Self {
        Self::Other {
            message: "Error in 'candle_core'.".to_string(),
            source: Some(Box::new(source)),
        }
    }
}
