use rayon::prelude::*;
use std::path::Path;

use crate::DatabaseError;
use duckdb::{Connection, Transaction, params};
use geo::Distance;
use linesonmaps::{
    algo::segmenter::segment_timestamp,
    types::{coordm::CoordM, linestringm::LineStringM, pointm::PointM},
};

pub fn update_db(db_path: &Path, file: &Path) -> Result<(), DatabaseError> {
    let mut conn = Connection::open(db_path)?;

    let tx = conn.transaction()?;
    if !file.is_file() {
        return Err(DatabaseError::FileDoesNotExist);
    }

    let path = file.canonicalize()?;
    let path_str = path.to_string_lossy();

    let count: i32 = tx.query_row(
        "SELECT count(*) FROM file_store WHERE path == ?",
        [path_str.clone()],
        |row| row.get(0),
    )?;
    if count != 0 {
        return Ok(());
    }

    tx.execute("INSERT INTO file_store VALUES (?)", [path_str])?;

    let mut stmt = tx.prepare("SELECT path FROM file_store")?;
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

fn dist(first: PointM, second: PointM, thres: f64) -> bool {
    use geo::algorithm::line_measures::metric_spaces::Geodesic;
    Geodesic.distance(first, second) < thres
}

const fn time_dist(first: PointM, second: PointM, thres: f64) -> bool {
    second.coord.m - first.coord.m < thres
}
