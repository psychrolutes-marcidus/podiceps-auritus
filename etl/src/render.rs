use std::cmp;
use std::sync::{Arc, Mutex};

use algorithms::cell::{gravity_model, st_tileenvelope};
use duckdb::{Config, Connection, DuckdbConnectionManager};
use duckdb::{Statement, params};
use geo::{Centroid, Geometry, Intersects, Point, algorithm};
use geo_traits::GeometryTrait;
use geo_traits::to_geo::{ToGeoGeometry, ToGeoLine, ToGeoPoint, ToGeoPolygon};
use r2d2::ManageConnection;
use rayon::prelude::*;
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{Envelope, RTreeObject};

use crate::RenderCell;

const EXTENSION_QUERY: &str = "LOAD '/home/rasmus/Projekter/xipeng/ferruginous/build/release/ferruginous.duckdb_extension'; LOAD spatial; SET geometry_always_xy = true;";

pub fn render_cells(params: RenderCell) {
    let config = Config::default()
        .allow_unsigned_extensions()
        .expect("Could not allow unsigned extensions");
    let con = Connection::open_with_flags(params.db_path.clone(), config)
        .expect("Could not open database");
    println!("Loading extension");
    con.execute_batch(EXTENSION_QUERY)
        .expect("Could not load extension");
    println!("Setup rendering views and tables");
    setup_rendering(&con, &params).expect("Could not setup rendering views and tables");
    println!("Getting candidate cells");
    con.close().expect("Could not close connection");
    let config = Config::default()
        .enable_autoload_extension(true)
        .expect("Cannot autoload extensions")
        .allow_unsigned_extensions()
        .expect("Cannot allow unsigned extensions")
        .access_mode(duckdb::AccessMode::ReadOnly)
        .expect("Cannot open in read only mode");
    let con = Connection::open_with_flags(params.db_path.clone(), config)
        .expect("Could not open connection pool");
    con.execute_batch(EXTENSION_QUERY)
        .expect("Could not load extensions");
    let candidate_cells =
        get_candidate_cells(con, &params).expect("Could not receive candidate cells");

    let config = Config::default()
        .allow_unsigned_extensions()
        .expect("Could not allow unsigned extensions");
    let con = Connection::open_with_flags(params.db_path.clone(), config)
        .expect("Could not open database");
    println!("Loading extension");
    con.execute_batch(EXTENSION_QUERY)
        .expect("Could not load extension");
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
    let query_start = "CREATE TEMP TABLE trajs AS (
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
        "CREATE TEMP TABLE lines AS (
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
    trajs ap
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
    nextval('geom_id_seq') as id,
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

CREATE INDEX geom_idx ON lines_with_geom USING RTREE (geom)";
    let sql = format!("LOAD spatial;
SET
  geometry_always_xy = TRUE;
  CREATE TEMP SEQUENCE geom_id_seq INCREMENT BY 1 START WITH 1;
CREATE OR REPLACE TABLE lines_with_geom AS (
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

type RectIdx = GeomWithData<Rectangle<Point>, usize>;
fn get_index(
    con: &Connection,
) -> Result<
    (
        rstar::RTree<GeomWithData<Rectangle<Point>, usize>>,
        Vec<Geometry>,
    ),
    Box<dyn std::error::Error>,
> {
    let mut stmt = con.prepare(
        "SELECT ST_AsWKB(ST_Transform(geom, 'EPSG:4326', 'EPSG:3857')) FROM lines_with_geom;",
    )?;
    let (aabbs, geoms): (Vec<_>, Vec<_>) = stmt
        .query_map([], |row| row.get::<_, Vec<u8>>(0))?
        .map(|x| x.unwrap())
        .enumerate()
        .map(|(i, geom)| {
            let geom = wkb::reader::read_wkb(&geom).expect("Malformed wkb");
            match geom.as_type() {
                geo_traits::GeometryType::Point(p) => (
                    RectIdx::new(
                        rstar::primitives::Rectangle::from_aabb(p.to_point().envelope()),
                        i,
                    ),
                    p.to_geometry(),
                ),
                geo_traits::GeometryType::Line(l) => (
                    RectIdx::new(
                        rstar::primitives::Rectangle::from_aabb(l.to_line().envelope()),
                        i,
                    ),
                    l.to_geometry(),
                ),
                geo_traits::GeometryType::Polygon(poly) => (
                    RectIdx::new(
                        rstar::primitives::Rectangle::from_aabb(poly.to_polygon().envelope()),
                        i,
                    ),
                    poly.to_geometry(),
                ),
                _ => unimplemented!(),
            }
        })
        .unzip();

    let index = rstar::RTree::bulk_load(aabbs);
    Ok((index, geoms))
}

pub fn get_candidate_cells(
    manager: Connection,
    params: &RenderCell,
) -> Result<Vec<(i32, i32)>, Box<dyn std::error::Error>> {
    let (index, geoms) = get_index(&manager)?;
    assert_ne!(geoms.len(), 0);
    let a_manager = Arc::new(Mutex::new(manager));
    let parser = |x: &String| {
        x.split(",")
            .flat_map(|x| x.parse::<i32>())
            .take(3)
            .collect()
    };
    let tile_start: Vec<_> = parser(&params.tile_start);

    assert_eq!(tile_start.len(), 3);

    // le.map(|x| x.0.ok().zip(x.1.ok()).zip(x.2.okdraughtmapscorex| medt  (draught, score, medcurrent_z_diff = params.level - tile_start[2];
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

    let mut cells: Vec<(i32, i32)> = (tile_start[0]..=tile_end[0])
        .map(|x| {
            (tile_start[1]..=tile_end[1])
                .zip(std::iter::repeat(x))
                .map(|(y, x)| (x, y))
        })
        .flatten()
        .collect();

    for i in tile_start[2]..=params.level {
        let increase = i < params.level;

        cells = cells
            .par_iter()
            .with_min_len(2048)
            .map(|x| {
                rayon::iter::repeat_n((x, increase as u32), 4_usize.pow(increase as u32))
                    .enumerate()
                    .map(|(i, (x, inc))| {
                        (
                            x.0 * 2_i32.pow(inc) + (i as i32) / 2,
                            x.1 * 2_i32.pow(inc) + (i as i32) % 2,
                        )
                    })
            })
            .flatten()
            .filter(|point| {
                let tile = st_tileenvelope(i as u32 + increase as u32, point.0, point.1);
                let mut inter = index.locate_in_envelope_intersecting(&tile.envelope());
                inter
                    .find_map(|x| match geoms[x.data].intersects(&tile) {
                        true => Some(x),
                        false => None,
                    })
                    .is_some()
            })
            .collect();
        println!("Level: {}, Cells: {}", i, cells.len());
    }

    //.map(|x| x.0.ok().zip(x.1.ok()).zip(x.2.okdraughtmapscorex| medle (draught, score, medt candidates: Vec<_> = cells
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
    Ok(cells)
}

fn render_cell_to_table(
    con: &Connection,
    cells: &[(i32, i32)],
    params: &RenderCell,
) -> Result<(), Box<dyn std::error::Error>> {
    let con = Arc::new(Mutex::new(
        con.try_clone().expect("Could not clone connection"),
    ));

    let chunks_size = cells.len() / 16;

    let query = "SELECT draught::float, render_geom(point, next_point, dimensions, {'x': ?, 'y': ?, 'level': ?}, parameters) as score, median_draught 
     FROM lines_with_geom b
     WHERE ST_Transform(ST_TileEnvelope(?, ?, ?), 'EPSG:3857', 'EPSG:4326') && geom
     ORDER BY draught, score DESC;";

    let result: Vec<_> = cells
        .par_chunks(chunks_size)
        .map(|x| {
            let con = con.lock().unwrap().try_clone().unwrap();
            con.execute_batch(EXTENSION_QUERY).unwrap();
            let mut stmt = con.prepare(query).unwrap();

            let something: Vec<_> = x
                .iter()
                .map(|(x, y)| {
                    let stuff: Vec<_> = stmt
                        .query_map(
                            params![
                                *x as u32,
                                *y as u32,
                                params.level as u8,
                                params.level,
                                *x,
                                *y
                            ],
                            |row| {
                                Ok((
                                    row.get::<_, f32>(0),
                                    row.get::<_, f32>(1),
                                    row.get::<_, f32>(2),
                                ))
                            },
                        )
                        .unwrap()
                        .flatten()
                        .map(|x| {
                            x.0.ok()
                                .zip(x.1.ok())
                                .zip(x.2.ok())
                                .map(|((draught, score), med)| (score, draught, med))
                        })
                        .flatten()
                        .collect();
                    let result = stuff
                        .iter()
                        .map(|left| {
                            stuff.iter().map(|right| {
                                (
                                    left.1,
                                    gravity_model(
                                        left.0, left.1, left.2, right.0, right.1, right.2,
                                    ),
                                )
                            })
                        })
                        .flatten()
                        .filter(|(_, rel)| *rel >= 0.53)
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or_default();
                    result
                })
                .collect();
            something
        })
        .flatten()
        .collect();

    // con.execute_batch(
    //     "LOAD spatial; SET geometry_always_xy = true;
    //     CREATE OR REPLACE TABLE cand_cells (
    //         id BIGINT,
    //         x INTEGER,
    //         y INTEGER,
    //         z INTEGER
    //     );
    //     CREATE OR REPLACE TABLE cand_cell_relation (
    //         cell_id BIGINT,
    //         geom_id BIGINT
    //     );
    //     ",
    // )?;

    // let mut cand_cell_app = con.appender_with_columns("cand_cells", &["id", "x", "y", "z"])?;
    // let mut relation_app =
    //     con.appender_with_columns("cand_cell_relation", &["cell_id", "geom_id"])?;

    // cells
    //     .iter()
    //     .enumerate()
    //     .for_each(|(cell_id, (cells, geom_ids))| {
    //         cand_cell_app
    //             .append_row(params![cell_id as i64 + 1, cells.0, cells.1, params.level])
    //             .expect("Could not append cand cell");
    //         geom_ids.iter().for_each(|id| {
    //             relation_app
    //                 .append_row([cell_id as i64, *id])
    //                 .expect("Could not append relation ids")
    //         });
    //     });

    // panic!("Done");

    //     let sql = "WITH
    //   scored AS MATERIALIZED (
    //     SELECT
    //       draught,
    //       render_geom (
    //         point,
    //         next_point,
    //         dimensions,
    //         {'x': ?, 'y': ?, 'level': ?},
    //         parameters
    //       ) as score,
    //       median_draught
    //     FROM
    //       lines_with_geom a
    //     WHERE
    //       draught IS NOT NULL AND ST_Transform(ST_TileEnvelope (?, ?, ?), 'EPSG:3857', 'EPSG:4326') && a.geom
    //   )
    // SELECT
    //   a.draught::float as draught,
    //   combine_cell (
    //     a.draught::float,
    //     a.score,
    //     a.median_draught::float,
    //     b.draught::float,
    //     b.score,
    //     b.median_draught::float
    //   ) as reliability
    // FROM
    //   scored a
    //   LEFT JOIN scored b ON a.draught >= b.draught
    // WHERE reliability >= 0.53
    // ORDER BY draught, reliability DESC
    // LIMIT 1;";

    //     let chunk_size = cmp::max(cells.len() / 16, 2048);
    //     let result: Vec<_> = cells
    //         .par_chunks(chunk_size)
    //         .map(|x| (x, con.lock().unwrap().try_clone().unwrap()))
    //         .map(|(cells, con)| {
    //             let mut stmt = con.prepare(sql).expect("Could not prepare statement");
    //             cells
    //                 .iter()
    //                 .map(|(x, y)| {
    //                     stmt.query_one(
    //                         params![
    //                             *x as u32,
    //                             *y as u32,
    //                             params.level as u8,
    //                             params.level,
    //                             *x,
    //                             *y
    //                         ],
    //                         |x| Ok((x.get::<_, f32>(0), x.get::<_, f32>(1))),
    //                     )
    //                 })
    //                 .collect::<Vec<_>>()
    //         })
    //         .flatten()
    //         .map(|x| x.ok())
    //         .map(|x| x.map(|x| (x.0.unwrap_or_default(), x.1.unwrap_or_default())))
    //         .map(|x| x.unwrap_or_default())
    //         .collect();

    // Write cells to table
    let con = con.lock().unwrap().try_clone().unwrap();
    con.execute_batch(
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

    let mut app = con.appender_to_db("render", "render")?;
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
