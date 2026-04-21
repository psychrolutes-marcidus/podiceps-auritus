use std::error::Error;

use duckdb::{Connection, duckdb_entrypoint_c_api};

pub mod etl;
mod eval;
pub mod render;

#[duckdb_entrypoint_c_api(ext_name = "ferruginous")]
pub fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn Error>> {
    etl::extension_entrypoint(&con)?;
    render::extension_entrypoint(&con)?;
    eval::extension_entrypoint(&con)?;
    Ok(())
}
