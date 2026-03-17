use std::path::Path;

use crate::DatabaseError;
use duckdb::Connection;

pub fn update_db(conn: &Connection, file: &Path) -> Result<(), DatabaseError> {
    if !file.is_file() {
        return Err(DatabaseError::FileDoesNotExist);
    }

    let path = file.canonicalize()?;
    let path_str = path.to_string_lossy();

    let count: i32 = conn.query_row(
        "SELECT count(*) FROM file_store WHERE path == ?",
        [path_str.clone()],
        |row| row.get(0),
    )?;
    if count != 0 {
        return Ok(());
    }

    conn.execute("INSERT INTO file_store VALUES (?)", [path_str])?;

    let mut stmt = conn.prepare("SELECT path FROM file_store")?;
    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let paths: Vec<_> = paths.iter().map(|x| format!("'{}'", x)).collect();

    let sql = format!(
        "CREATE OR REPLACE VIEW ais_data AS (SELECT * FROM read_parquet([{}]))",
        paths.join(",")
    );

    conn.execute(sql.as_str(), [])?;
    Ok(())
}
