use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("database error: {0}")]
    DatabaseError(#[from] database::Error),
}
