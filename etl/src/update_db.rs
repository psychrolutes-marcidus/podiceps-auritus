use rayon::prelude::*;
use std::{fs, path::Path};
use sysinfo::System;

use crate::DatabaseError;
use duckdb::{Connection, Transaction, params};
use geo::Distance;
use linesonmaps::{
    algo::segmenter::segment_timestamp,
    types::{coordm::CoordM, linestringm::LineStringM, pointm::PointM},
};

pub fn update_db(db_path: &Path, path: &Path) -> Result<(), DatabaseError> {
    let mut conn = Connection::open(db_path)?;
    let mem = get_system_memory();

    let tx = conn.transaction()?;
    let sql = format!("SET memory_limit = '{mem}GB';");
    dbg!(&sql);
    tx.execute(&sql, [])?;

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
    paths.iter().for_each(|path|{
        
    let sql = format!(
        "INSERT INTO ais_data SELECT DISTINCT ON (mmsi, timestamp) mmsi, timestamp, lat, lon, ship_length, ship_width, draught, to_bow, to_starboard, to_stern, to_port, ship_type FROM read_parquet({}) WHERE lat != 91 ORDER BY ALL;",
        path
    );

    tx.execute(sql.as_str(), []).expect("Could not import data");
    });

    update_tables(&tx)?;
    tx.commit()?;
    Ok(())
}

fn update_tables(tx: &Transaction) -> Result<(), DatabaseError> {
    tx.execute_batch("CREATE OR REPLACE TABLE main.length_confidence AS (
    SELECT
        ship_type,
        min(ship_length) AS mi,
        max(ship_length) AS ma,
        QUANTILE_DISC(ship_length, [0.01, 0.99]) AS confidence,
        count(ship_length) AS num_lengths,
        count(DISTINCT ship_length) AS distinct_lengths
    FROM
        main.ais_data
    GROUP BY
        ship_type
);

CREATE OR REPLACE TABLE main.width_confidence AS (
    SELECT
        ship_type,
        min(ship_width) AS mi,
        max(ship_width) AS ma,
        QUANTILE_DISC(ship_width, [0.01, 0.99]) AS confidence,
        count(ship_width) AS num_widths,
        count(DISTINCT ship_width) AS distinct_widths
    FROM
        main.ais_data
    GROUP BY
        ship_type
);

CREATE OR REPLACE TABLE vessel_stats.linear_regression AS (
    SELECT
        lc.ship_type,
        REGR_SLOPE(ad.draught, ad.ship_length) AS slope, -- growth in draught as a function of ship length
        REGR_INTERCEPT(ad.draught, ad.ship_length) AS intercept, -- draught-offset at ship_length=0
        REGR_R2(ad.draught, ad.ship_length) AS r_squared,
        count(*) num_messages
    FROM
        main.ais_data AS ad
        JOIN length_confidence lc ON lc.ship_type = ad.ship_type
        JOIN width_confidence wc ON wc.ship_type = ad.ship_type
        JOIN main.confidence_by_vessel vc ON vc.ship_type = ad.ship_type
    WHERE
        ad.ship_length BETWEEN lc.confidence[1] AND lc.confidence[2]
        AND ad.draught BETWEEN vc.confidence[1] AND vc.confidence[2]
        AND ad.ship_width BETWEEN wc.confidence[1] AND wc.confidence[2]
        AND ad.lat != 91 -- REGR_{SLOPE | INTERCEPT | R2} ignore null values
    GROUP BY
        lc.ship_type
);

CREATE OR REPLACE TABLE vessel_stats.std_draught AS (
    SELECT
        mmsi,
        STDDEV_POP(draught) AS sd_draught,
        ABS(STDDEV_POP(draught) / AVG(draught)) AS rsd_avg_draught,
        ABS(STDDEV_POP(draught) / MEDIAN(draught)) AS rsd_median_draught,
        MAD(draught) as mad
    FROM
        main.ais_data
    GROUP BY
        mmsi
);
")?;
    Ok(())
}

fn get_system_memory() -> String {
    let sys = System::new_all();
    let main_mem = sys.total_memory();
    let swap_mem = sys.total_swap();
    let mem = main_mem.max(swap_mem) / 5 * 3;
    let mem_gb = mem as f64 / 1024. / 1024. / 1024.;
    mem_gb.to_string()
}
