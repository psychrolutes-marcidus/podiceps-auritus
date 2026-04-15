use std::{
    backtrace::Backtrace,
    path::{Path, PathBuf},
    process::exit,
};
use thiserror::Error;

use clap::Parser;

use crate::{new_db::create_db, update_db::update_db, update_ddm::update_ddm};

mod new_db;
mod update_db;
mod update_ddm;

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
    UpdateDDM(UpdateDDM),
}

#[derive(clap::Args, Debug)]
struct Update {
    #[arg(short, long)]
    db_path: PathBuf,
    #[arg(short, long)]
    import_file: Option<PathBuf>,
    #[arg(short, long)]
    import_directory: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct UpdateDDM {
    #[arg(long)]
    db_path: PathBuf,
    #[arg(long)]
    ddm_file: PathBuf,
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
            let path = match update.import_file.xor(update.import_directory) {
                Some(p) => p,
                None => {
                    println!("A file OR a directory must be set");
                    exit(1)
                }
            };
            update_db(&update.db_path, &path).unwrap();
        }
        Args::UpdateDDM(ddm_update) => {
            let path = Path::new(&ddm_update.ddm_file);
            update_ddm(&ddm_update.db_path, path).unwrap();
        }
    }
}
