use std::error::Error;

use duckdb::{duckdb_entrypoint_c_api, Connection};

pub mod etl;
pub mod render;

#[duckdb_entrypoint_c_api(ext_name = "ferruginous")]
pub fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn Error>> {
    etl::extension_entrypoint(&con)?;
    render::extension_entrypoint(&con)?;
    eval::extension_entrypoint(&con)?;
    Ok(())
}
