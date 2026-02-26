use linesonmaps::types::error::Error as LomError;
use postgres::Error as PgError;
use std::env::VarError;
use std::net::AddrParseError;
use std::num::ParseIntError;
use thiserror::Error;
use wkb::error::WkbError;

#[derive(Error, Debug)]
pub enum DataError {
    #[error("Database Error")]
    Database(#[from] DatabaseError),
    #[error("Table Error")]
    Table(#[from] TableError),
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database connection error")]
    Connect(PgError),
    #[error("Database query error: {msg}, {db_error}")]
    QueryError { db_error: PgError, msg: String },
    #[error("Invalid Port")]
    PortParse(#[from] ParseIntError),
    #[error("Invalid IP Address")]
    IpAddrParse(#[from] AddrParseError),
    #[error("Missing Environment Variables")]
    MissingEnv(#[from] VarError),
    #[error("Invalid WKB")]
    WKBParse(#[from] WkbError),
    #[error("Linestring creation")]
    LinestringParse(#[from] LomError),
    #[error("writer error")]
    IoError(#[from]std::io::Error),
}

#[derive(Error, Debug)]
pub enum TableError {
    #[error("Could not find key in table")]
    MissingKey,
    #[error("Key already exists in table")]
    DuplicateKey,
    #[error("Error loading data into table")]
    LoaderError,
}
