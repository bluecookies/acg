#[macro_export]
macro_rules! include_query {
    ($file:expr $(,)?) => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/queries/", $file)))
}

macro_rules! prepare_statement {
    ($client:expr, $query_file:literal, $name:expr) => (
        $client.prepare_cached(include_query!($query_file))
            .await
            .map_err(|e| crate::Error::PrepareStatement($name, e))
    );
    ($client:expr, $query:expr, $name:expr) => (
        $client.prepare_cached($query)
            .await
            .map_err(|e| crate::Error::PrepareStatement($name, e))
    );
    ($client:expr, $query_file:literal, $name:expr, $($ty:ident),+,) => (
        $client.prepare_typed_cached(include_query!($query_file), &[$(tokio_postgres::types::Type::$ty),+])
            .await
            .map_err(|e| crate::Error::PrepareStatement($name, e))
    );
}

mod database;
mod song;
mod stats;
mod error;

pub use database::Database;
pub use song::{SongData, SongInfo, SearchQuery};
pub use error::Error;

pub use tokio_postgres::types::ToSql;
