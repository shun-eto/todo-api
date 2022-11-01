pub mod label;
pub mod todo;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Duplicate data, id is {0}")]
    Duplicate(i32),
    #[error("Unexpected Error: [{0}]")]
    UnexpectedError(String),
    #[error("NotFound, id is {0}")]
    NotFound(i32),
}
