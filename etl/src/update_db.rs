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
    update_trajectories(&tx)?;
    tx.commit()?;
    Ok(())
}

pub fn update_trajectories(tx: &Transaction) -> Result<(), DatabaseError> {
    tx.execute_batch(
        "
CREATE OR REPLACE TEMP TABLE temp_traj AS
    (SELECT *
     FROM latest_trajectories);


CREATE OR REPLACE TEMP TABLE temp_search_points AS
    (SELECT DISTINCT ON (mmsi) mmsi,
                        time_begin
     FROM
         (SELECT mmsi,
                 time_begin
          FROM oldest_message
          UNION SELECT mmsi,
                       time_begin
          FROM temp_traj)
     ORDER BY time_begin DESC);
     ",
    )?;

    let mut point_pre = tx.prepare(
        "
    SELECT mmsi,
           epoch(timestamp) AS timestamp,
           lon,
           lat
    FROM ais_point ap
    WHERE timestamp >=
            (SELECT time_begin
             FROM temp_search_points tsp
             WHERE ap.mmsi = tsp.mmsi)
    ORDER BY mmsi, timestamp
             ",
    )?;

    let something = point_pre.query_map([], |row| {
        Ok(row
            .get(0)
            .ok()
            .zip(row.get(1).ok())
            .zip(row.get(2).ok())
            .zip(row.get(3).ok())
            .map(|(((a, b), c), d)| (a, b, c, d)))
    })?;

    let mut x = Vec::new();
    for e in something {
        match e? {
            Some(el) => x.push(el),
            None => {}
        }
    }

    let func = |f, l| dist(f, l, 1000_f64) && time_dist(f, l, 60_f64);

    let segments: Vec<_> = x
        .par_chunk_by(|a, b| a.0 == b.0)
        .map(|x| row_to_line(x))
        .flatten()
        .map(|(m, ls)| {
            segment_timestamp(ls, func)
                .into_iter()
                .filter(|(_, d)| !d.is_zero())
                .map(|(t, d)| (m, t, d))
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect();
    let mut app = tx.appender_with_columns("trajectories", &["mmsi", "time_begin", "duration"])?;

    let _ = segments
        .iter()
        .map(|row| app.append_row(params![row.0, row.1, row.2]))
        .collect::<Result<Vec<_>, _>>()?;
    tx.execute(
        "
DELETE
FROM trajectories t
WHERE id IN
        (SELECT id
         FROM temp_traj tt
         WHERE t.id = tt.id)
         ",
        [],
    )?;

    Ok(())
}

fn row_to_line(rows: &[(i64, f64, f32, f32)]) -> Option<(i64, LineStringM)> {
    let mmsi = rows.first().map(|x| x.0).expect("This should not be empty");

    let coords: Vec<CoordM<4326>> = rows
        .iter()
        .map(|(_, t, lon, lat)| CoordM {
            x: *lon as f64,
            y: *lat as f64,
            m: *t,
        })
        .collect();

    LineStringM::new(coords).map(|ls| (mmsi, ls))
}

fn dist(first: PointM, second: PointM, thres: f64) -> bool {
    use geo::algorithm::line_measures::metric_spaces::Geodesic;
    Geodesic.distance(first, second) < thres
}

const fn time_dist(first: PointM, second: PointM, thres: f64) -> bool {
    second.coord.m - first.coord.m < thres
}
