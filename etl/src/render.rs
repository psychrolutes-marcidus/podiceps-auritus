use duckdb::{Config, Connection};
use duckdb::{Statement, params};

use crate::RenderCell;

pub fn render_cells(params: RenderCell) {
    let config = Config::default()
        .allow_unsigned_extensions()
        .expect("Could not allow unsigned extensions");
    let con = Connection::open_with_flags(params.db_path.clone(), config)
        .expect("Could not open database");
    println!("Loading extension");
    con.execute_batch("LOAD '/home/rasmus/Projekter/xipeng/ferruginous/build/release/ferruginous.duckdb_extension';").expect("Could not load extension");
    println!("Setup rendering views and tables");
    setup_rendering(&con, &params).expect("Could not setup rendering views and tables");
    println!("Getting candidate cells");
    let candidate_cells =
        get_candidate_cells(&con, &params).expect("Could not receive candidate cells");
    println!("Rendering cells to table");
    render_cell_to_table(&con, &candidate_cells, &params).expect("Could not render cells to table");
}

pub fn setup_rendering(
    tx: &Connection,
    params: &RenderCell,
) -> Result<(), Box<dyn std::error::Error>> {
    tx.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS draught_dist_mmsi_normal AS (
  SELECT *
  FROM render.draught_dist_mmsi_normal
);
CREATE TEMP TABLE IF NOT EXISTS draught_dist_vessel_type_normal AS (
  SELECT *
  FROM render.draught_dist_vessel_type_normal
);
CREATE TEMP TABLE IF NOT EXISTS draught_nulls_by_ship_type AS (
  SELECT *
  FROM vessel_attributes.draught_nulls_by_ship_type
);",
    )?;
    let query_start = "CREATE OR REPLACE VIEW render.trajs AS (
      SELECT
        ap.mmsi,
        ap.timestamp,
        {'lon': lon, 'lat': lat, 'time': epoch(ap.timestamp)} as point,
        {
              'to_bow': ap.to_bow::float,
              'to_starboard': ap.to_starboard::float,
              'to_stern': ap.to_stern::float,
              'to_port': ap.to_port::float
      } as dimensions,
      draught,
      ship_type
      FROM
        ais_data ap";
    let query = format!(
        "{}
        WHERE ap.timestamp >= '{}' AND ap.timestamp <= '{}'
        );",
        query_start, params.time_start, params.time_stop
    );
    tx.execute_batch(&query)?;

    tx.execute_batch(
        "CREATE OR REPLACE VIEW lines AS (
  SELECT
    ap.mmsi,
    ap.timestamp,
    ap.point,
    CASE
      WHEN LEAD (ap.timestamp) OVER (
      PARTITION BY mmsi
        ORDER BY
          ap.mmsi,
          ap.timestamp
      ) > ap.timestamp
      AND trajectory_split (
        ap.point,
        LEAD (ap.point, 1, NULL) OVER (
          PARTITION BY
            mmsi
          ORDER BY
            ap.mmsi,
            ap.timestamp
        )
      )
      AND LEAD (ap.mmsi) OVER (
        PARTITION BY mmsi
        ORDER BY
          ap.mmsi,
          ap.timestamp
      ) = ap.mmsi
      AND (
        LEAD (ap.point) OVER (
          PARTITION BY
            mmsi
          ORDER BY
            ap.mmsi,
            ap.timestamp
        ).lat != ap.point.lat
        OR LEAD (ap.point) OVER (
          PARTITION BY
            mmsi
          ORDER BY
            ap.mmsi,
            ap.timestamp
        ).lon != ap.point.lon
      ) THEN LEAD (ap.point, 1, NULL) OVER (
        PARTITION BY mmsi
        ORDER BY
          ap.mmsi,
          ap.timestamp
      )
      ELSE NULL
    END AS next_point,
    dimensions,
    draught,
    ship_type
  FROM
    render.trajs ap
);
",
    )?;

    println!("Polygonise lines");

    let parser = |x: &String| {
        x.split(",")
            .flat_map(|x| x.parse::<i32>())
            .take(3)
            .collect()
    };
    let tile_start: Vec<_> = parser(&params.tile_start);
    let tile_end: Vec<_> = parser(&params.tile_end.as_ref().unwrap_or(&params.tile_start));

    let rest = "
  SELECT
    ap.mmsi,
    ap.timestamp,
    ap.point,
    ap.next_point,
    median_draught,
    {
      'draught_dist_mmsi': cbv.score_norm::float,
      'draught_dist_type': cbm.score_norm::float,
      'draughts_null': dnull.draughts_null::float,
      'r_squared': lr.r_squared::float
    } as parameters,
    CASE
      WHEN ap.next_point IS NOT NULL
      AND {'no': 1} IN (dimensions) IS NOT NULL
      THEN st_geomfromwkb (polyganize (ap.point, ap.next_point, dimensions))
      WHEN ap.next_point IS NOT NULL
      AND {'no': 1} IN (dimensions) IS NULL THEN ST_MakeLine (
        ST_Point (ap.point.lon, ap.point.lat),
        ST_Point (ap.next_point.lon, ap.next_point.lat)
      )
      ELSE ST_Point (ap.point.lon, ap.point.lat)
    END as geom,
    dimensions,
    ap.draught
  FROM
    lines ap
    LEFT JOIN draught_dist_mmsi_normal cbm ON ap.mmsi = cbm.mmsi
    AND ap.draught = cbm.draught
    LEFT JOIN draught_dist_vessel_type_normal cbv ON ap.ship_type = cbv.ship_type
    AND ap.draught = cbv.draught
    LEFT JOIN draught_nulls_by_ship_type dnull ON ap.ship_type = dnull.ship_type
    LEFT JOIN vessel_stats.linear_regression lr ON ap.ship_type = lr.ship_type
    LEFT JOIN vessel_stats.std_draught sd ON ap.mmsi = sd.mmsi
    WHERE 
    (SELECT true FROM cand_cells b WHERE ST_Intersects(cellgeom, geom) LIMIT 1)
);

CREATE INDEX vessel_idx ON lines_with_geom USING rtree(geom);";
    let sql = format!("LOAD spatial;
SET
  geometry_always_xy = TRUE;
CREATE OR REPLACE TEMP TABLE lines_with_geom AS (
  WITH cand_cells AS MATERIALIZED (
SELECT
              xt.* as x,
              yt.* as y,
              {} as z,
              ST_Transform(ST_TileEnvelope (z::integer, x::integer, y::integer), 'EPSG:3857', 'EPSG:4326') as cellgeom
            FROM
              generate_series({}, {}, 1) xt,
              generate_series({}, {}, 1) yt
              )
              {}", tile_start[2], tile_start[0], tile_end[0], tile_start[1], tile_end[1], rest);
    tx.execute_batch(&sql)?;

    Ok(())
}

pub fn get_candidate_cells(
    tx: &Connection,
    params: &RenderCell,
) -> Result<Vec<(i32, i32)>, Box<dyn std::error::Error>> {
    let parser = |x: &String| {
        x.split(",")
            .flat_map(|x| x.parse::<i32>())
            .take(3)
            .collect()
    };
    let tile_start: Vec<_> = parser(&params.tile_start);

    assert_eq!(tile_start.len(), 3);

    // let current_z_diff = params.level - tile_start[2];
    // tile_start[0] = tile_start[0] * 2_i32.pow(current_z_diff as u32);
    // tile_start[1] = tile_start[1] * 2_i32.pow(current_z_diff as u32);
    // tile_start[2] = params.level;

    // let tile_ender = |tile_end: Vec<i32>| {
    //     let mut tile_end = tile_end.clone();
    //     let current_z_diff = params.level - tile_end[2];
    //     tile_end[0] = (tile_end[0] + 1) * 2_i32.pow(current_z_diff as u32);
    //     tile_end[1] = (tile_end[1] + 1) * 2_i32.pow(current_z_diff as u32);
    //     tile_end[2] = params.level;
    //     tile_end
    // };

    let tile_end: Vec<_> = parser(&params.tile_end.clone().unwrap_or(params.tile_start.clone()));

    let cells: Vec<(i32, i32)> = (tile_start[0]..=tile_end[0])
        .map(|x| {
            (tile_start[1]..=tile_end[1])
                .zip(std::iter::repeat(x))
                .map(|(y, x)| (x, y))
        })
        .flatten()
        .collect();

    let query = "SELECT count(*) FROM lines_with_geom WHERE ST_Intersects(ST_Transform(ST_TileEnvelope(?, ?, ?), 'EPSG:3857', 'EPSG:4326'), geom) LIMIT 1";
    let mut stmt = tx.prepare(&query)?;

    let candidates = cell_checker(&mut stmt, cells, tile_start[2], params.level);
    // let candidates: Vec<_> = cells
    //     .map(|(x, y)| {
    //         (
    //             x,
    //             y,
    //             stmt.query_row([params.level, x, y], |row| row.get::<_, i32>(0))
    //                 .unwrap(),
    //         )
    //     })
    //     .filter(|x| x.2 != 0)
    //     .map(|(x, y, _)| (x, y))
    //     .collect();
    Ok(candidates)
}

fn cell_checker(
    stmt: &mut Statement,
    candidates: Vec<(i32, i32)>,
    current_level: i32,
    end_level: i32,
) -> Vec<(i32, i32)> {
    if current_level > end_level {
        return candidates;
    }
    let increase = current_level < end_level;
    let new_candidates: Vec<_> = candidates
        .iter()
        .zip(std::iter::repeat(increase))
        .map(|(x, i)| {
            std::iter::repeat_n((x, i as u32), 4_usize.pow(i as u32))
                .enumerate()
                .map(|(i, (x, inc))| {
                    (
                        x.0 * 2_i32.pow(inc) + (i as i32) / 2,
                        x.1 * 2_i32.pow(inc) + (i as i32) % 2,
                    )
                })
        })
        .flatten()
        .zip(std::iter::repeat(current_level))
        .zip(std::iter::repeat(increase))
        .filter(|((x, level), inc)| {
            let count = stmt
                .query_row([level + *inc as i32, x.0, x.1], |row| row.get::<_, i64>(0))
                .unwrap();
            count > 0
        })
        .map(|((x, _), _)| x)
        .collect();
    dbg!(&new_candidates.len());
    if current_level + 1 >= end_level {
        return new_candidates;
    }
    return cell_checker(stmt, new_candidates, current_level + 1, end_level);
}

fn render_cell_to_table(
    tx: &Connection,
    cells: &[(i32, i32)],
    params: &RenderCell,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stmt = tx.prepare(
        "WITH
  scored AS (
    SELECT
      draught,
      render_geom (
        point,
        next_point,
        dimensions,
        {'x': ?, 'y': ?, 'level': ?},
        parameters
      ) as score,
      median_draught
    FROM
      lines_with_geom a
    WHERE
      draught IS NOT NULL AND ST_Transform(ST_TileEnvelope (?, ?, ?), 'EPSG:3857', 'EPSG:4326') && a.geom
  )
SELECT
  a.draught::float as draught,
  combine_cell (
    a.draught::float,
    a.score,
    a.median_draught::float,
    b.draught::float,
    b.score,
    b.median_draught::float
  ) as reliability
FROM
  scored a
  LEFT JOIN scored b ON a.draught >= b.draught
WHERE reliability >= 0.53
ORDER BY draught, reliability DESC
LIMIT 1;",
    )?;
    let result: Vec<_> = cells
        .iter()
        .map(|(x, y)| {
            stmt.query_one(
                params![
                    *x as u32,
                    *y as u32,
                    params.level as u8,
                    params.level,
                    *x,
                    *y
                ],
                |x| Ok((x.get::<_, f32>(0), x.get::<_, f32>(1))),
            )
        })
        .map(|x| x.ok())
        .map(|x| x.map(|x| (x.0.unwrap_or_default(), x.1.unwrap_or_default())))
        .map(|x| x.unwrap_or_default())
        .collect();

    // Write cells to table
    tx.execute_batch(
        "
        CREATE OR REPLACE TABLE render.render (
                x INTEGER,
                y INTEGER,
                z INTEGER,
                draught FLOAT,
                reliability FLOAT
            );
        ",
    )?;

    let mut app = tx.appender_to_db("render", "render")?;
    let result: Result<Vec<_>, _> = cells
        .iter()
        .zip(result.iter())
        .map(|((x, y), (draught, rely))| {
            app.append_row(params![*x, *y, params.level, *draught, *rely])
        })
        .collect();
    result?;
    Ok(())
}
