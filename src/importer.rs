//! Data sources that produce a [`Dataset`].
//!
//! The CSV importer is one implementation; a future web grabber that scrapes
//! spritmonitor.de directly only needs to implement [`DatasetSource`] — the
//! analytics and web layers never see where the data came from.

pub mod csv;

use crate::domain::Dataset;

/// A source of one vehicle's fueling history.
pub trait DatasetSource {
    /// Fetches and normalizes the data into the canonical domain model.
    ///
    /// Row-level problems become [`crate::domain::ImportWarning`]s inside a
    /// successful dataset; only structural failure is an error.
    fn fetch(&self) -> impl std::future::Future<Output = Result<Dataset, SourceError>> + Send;
}

/// Structural failure of an import — nothing usable came out of the source.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("no recognizable header row found")]
    NoRecognizableHeader,
    #[error("file contains no valid fueling rows")]
    NoValidRows,
    #[error("csv parse error: {0}")]
    Csv(String),
}

impl SourceError {
    /// Translation key for the localized error message.
    pub fn i18n_key(&self) -> &'static str {
        match self {
            Self::NoRecognizableHeader => "error.no_header",
            Self::NoValidRows => "error.no_rows",
            Self::Csv(_) => "error.csv",
        }
    }
}
