use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("database not initialised")]
    NoDatabase,
    #[error("could not get postgres client: {0}")]
    ClientGetError(#[from] deadpool_postgres::PoolError),
    #[error("failed to prepare SQL statement for {0}: {1}")]
    PrepareStatement(&'static str, tokio_postgres::Error),
    #[error("query failed for {0}: {1}")]
    QueryError(&'static str, tokio_postgres::Error),
    #[error("postgres error: {0}")]
    PostgresError(#[from] tokio_postgres::Error),
    // #[error("deserialise error: {0}")]
    // DeserialiseError(#[from] crate::serde_postgres::DeError)
    #[error("query type error: {0}")]
    TypeError(tokio_postgres::Error),
}
