use rayon::prelude::*;
use std::{fs, path::Path};

use crate::DatabaseError;
use duckdb::{Connection, Transaction, params};
use geo::Distance;
use linesonmaps::{
    algo::segmenter::segment_timestamp,
    types::{coordm::CoordM, linestringm::LineStringM, pointm::PointM},
};

pub fn update_db(db_path: &Path, path: &Path) -> Result<(), DatabaseError> {
    let mut conn = Connection::open(db_path)?;

    let tx = conn.transaction()?;

    let path_strs = match path.is_dir() {
        true => {
            // Read all files in the directory
            let path = path.canonicalize()?;
            let paths = fs::read_dir(path)?;
            paths
                .map(|x| x.ok())
                .flatten()
                .map(|x| x.path().display().to_string())
                .collect()
        }
        false => {
            let path = path.canonicalize()?;
            vec![path.to_string_lossy().to_string()]
        }
    };
    for ele in path_strs {
        let count: i32 = tx.query_row(
            "SELECT count(*) FROM file_store WHERE path = ?",
            [ele.clone()],
            |row| row.get(0),
        )?;
        if count == 0 {
        tx.execute("INSERT INTO file_store VALUES (?)", [ele])?;
        }
    }

    let mut stmt = tx.prepare("SELECT path FROM file_store ORDER BY path")?;
    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let paths: Vec<_> = paths.iter().map(|x| format!("'{}'", x)).collect();

    let sql = format!(
        "CREATE OR REPLACE VIEW ais_data AS (SELECT * FROM read_parquet([{}]) WHERE lat != 91)",
        paths.join(",")
    );

    tx.execute(sql.as_str(), [])?;
    tx.commit()?;
    Ok(())
}
