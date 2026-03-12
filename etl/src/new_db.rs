use crate::DatabaseError;
use duckdb::{Connection, Transaction};
use std::{
    fs::{self, read_dir},
    path::Path,
};

pub fn create_db(path: &Path) -> Result<(), DatabaseError> {
    let mut conn = Connection::open(path)?;
    let tx = conn.transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS migration (last_migration TEXT);")?;
    setup_schema(&tx)?;
    tx.commit()?;
    Ok(())
}

pub fn setup_schema(tx: &Transaction) -> Result<(), DatabaseError> {
    // Run all migrations in migration folder.
    let current_mig: Option<String> = tx
        .query_row(
            "DELETE FROM migration RETURNING last_migration;",
            [],
            |row| row.get(0),
        )
        .ok();
    let migration_folder = Path::new("etl/src/sql/migration");
    let abs_path = migration_folder.canonicalize()?;
    let dir = read_dir(abs_path)?;
    let mut entries = dir
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    let pos = entries
        .iter()
        .rposition(|x| current_mig.clone().unwrap_or(String::new()) == x.to_string_lossy())
        .map(|x| x + 1);

    let sql_queries: Vec<String> = entries
        .iter()
        .skip(pos.unwrap_or(0))
        .map(|x| fs::read_to_string(x))
        .collect::<Result<_, _>>()?;
    sql_queries
        .iter()
        .map(|query| tx.execute_batch(query))
        .collect::<Result<Vec<_>, _>>()?;

    match entries.iter().skip(pos.unwrap_or(0)).last() {
        Some(e) => {
            tx.execute("INSERT INTO migration VALUES (?)", [e.to_string_lossy()])?;
        }
        None => {
            tx.execute(
                "INSERT INTO migration VALUES (?)",
                [current_mig.unwrap_or(String::new())],
            )?;
        }
    }
    Ok(())
}
