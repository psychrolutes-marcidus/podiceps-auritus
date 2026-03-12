use crate::DatabaseError;
use duckdb::Connection;
use std::{
    fs::{self, read_dir},
    path::Path,
};

pub fn create_db(path: &Path) -> Result<Connection, DatabaseError> {
    let conn = Connection::open(path)?;
    conn.execute_batch("BEGIN; CREATE TABLE IF NOT EXISTS migration (last_migration TEXT); INSERT INTO migration VALUES (''); COMMIT;")?;
    setup_schema(&conn)?;
    Ok(conn)
}

pub fn setup_schema(conn: &Connection) -> Result<(), DatabaseError> {
    // Run all migrations in migration folder.
    let current_mig: String =
        conn.query_row("SELECT last_migration FROM migration;", [], |row| {
            row.get(0)
        })?;
    let migration_folder = Path::new("etl/src/sql/migration");
    let abs_path = migration_folder.canonicalize()?;
    let dir = read_dir(abs_path)?;
    let mut entries = dir
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    dbg!(&entries);
    let pos = entries
        .iter()
        .rposition(|x| x.to_string_lossy() == current_mig)
        .map(|x| x + 1);

    let sql_queries: Vec<String> = entries
        .iter()
        .skip(pos.unwrap_or(0))
        .map(|x| fs::read_to_string(x))
        .collect::<Result<_, _>>()?;
    dbg!(&sql_queries);
    sql_queries
        .iter()
        .map(|query| conn.execute_batch(query))
        .collect::<Result<Vec<_>, _>>()?;

    match entries.iter().skip(pos.unwrap_or(0)).last() {
        Some(e) => {
            conn.execute(
                "UPDATE migration SET last_migration = ?",
                [e.to_string_lossy()],
            )?;
        }
        None => {}
    }
    Ok(())
}
