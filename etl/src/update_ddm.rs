use std::path::Path;

use crate::DatabaseError;
use duckdb::Connection;

pub fn update_ddm(db_path: &Path, file: &Path) -> Result<(), DatabaseError> {
    let mut conn = Connection::open(db_path)?;

    let tx = conn.transaction()?;
    if !file.is_file() {
        return Err(DatabaseError::FileDoesNotExist);
    }

    let path = file.canonicalize()?;
    let path_str = path.to_string_lossy();

    tx.execute_batch(&format!(
        "
            INSTALL spatial;
            LOAD spatial;
            CREATE OR REPLACE TABLE ddm AS
            SELECT
                depth,
                CASE source
                    WHEN 3 THEN 0
                    WHEN 2 THEN 1
                    WHEN 4 THEN 2
                    WHEN 5 THEN 3
                    WHEN 6 THEN 4
                    WHEN 1 THEN 5
                    WHEN 7 THEN 6
                    WHEN 8 THEN 7
                END AS source,
                year,
                ST_Transform (geom, 'EPSG:3034', 'EPSG:3857', always_xy := TRUE) AS geom
            FROM
                read_parquet('{path_str}');
            DROP INDEX IF EXISTS ddm_idx;
            CREATE INDEX ddm_idx ON ddm USING RTREE (geom);
        "
    ))?;

    tx.commit()?;
    Ok(())
}
