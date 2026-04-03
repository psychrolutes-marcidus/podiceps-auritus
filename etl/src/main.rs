use std::{
    backtrace::Backtrace,
    path::{Path, PathBuf},
};
use thiserror::Error;

use clap::Parser;

use crate::{new_db::create_db, update_db::update_db};

mod new_db;
mod update_db;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("DuckDB error")]
    DuckDBError(#[from] duckdb::Error),
    #[error("IO error")]
    Io(#[from] std::io::Error),
    #[error("Imported file does not exist")]
    FileDoesNotExist,
}
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
enum Args {
    NewDatabase(NewDatabase),
    UpdateDatabase(Update),
}

#[derive(clap::Args, Debug)]
struct Update {
    #[arg(short, long)]
    db_path: PathBuf,
    #[arg(short, long)]
    import_file: PathBuf,
}

#[derive(clap::Args, Debug)]
struct NewDatabase {
    db_path: String,
}

fn main() {
    match Args::parse() {
        Args::NewDatabase(new_database) => {
            let path = PathBuf::from(new_database.db_path);
            let _conn = create_db(&path).unwrap();
        }
        Args::UpdateDatabase(update) => {
            let path = Path::new(&update.import_file);
            update_db(&update.db_path, path).unwrap();
        }
    }
}
