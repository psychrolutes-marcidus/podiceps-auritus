use chrono::prelude::*;
use data::tables::Ships;
use geo_types::Coord;
use itertools::Itertools;
use linesonmaps::types::{coordm::CoordM, linestringm::LineStringM};
// use pgrx::prelude::*;
use rayon::prelude::*;
// use serde::{Deserialize, Serialize};
use std::{cmp, sync::Arc};

pub mod tile3d;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy,Hash, /* Serialize, Deserialize */)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, /* Serialize, Deserialize */)]
pub struct PointWZ {
    pub point: Point,
    pub z: i32,
}

impl Zoom for PointWZ {
    fn change_zoom(self, zoom_level: i32) -> Self {
        let change = self.z - zoom_level;
        let x;
        let y;

        if change > 0 {
            x = self.point.x / 2_i32.pow(change.abs() as u32);
            y = self.point.y / 2_i32.pow(change.abs() as u32);
        } else {
            x = self.point.x / 2_i32.pow(change.abs() as u32);
            y = self.point.y / 2_i32.pow(change.abs() as u32);
        }

        Self {
            point: Point { x: x, y: y },
            z: zoom_level,
            ..self
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone,Hash, /* Serialize, Deserialize */)]
pub struct PointWTime {
    pub point: Point,
    pub z: i32,
    pub time_start: DateTime<Utc>,
    pub time_end: DateTime<Utc>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub max_draught: Option<f32>,
    pub distinct_ship_count: u64,
    pub min_sog: Option<f32>,
    pub max_sog: Option<f32>,
    pub cell_oc_time: chrono::TimeDelta,
    pub min_length: Option<f64>,
    pub max_length: Option<f64>,
    pub min_width: Option<f64>,
    pub max_width: Option<f64>,
}

pub trait Zoom {
    fn change_zoom(self, zoom_level: i32) -> Self;
}

impl Zoom for Tile {
    fn change_zoom(self, zoom_level: i32) -> Self {
        let change = self.z - zoom_level;
        let x;
        let y;

        if change > 0 {
            x = self.x / 2_i32.pow(change.abs() as u32);
            y = self.y / 2_i32.pow(change.abs() as u32);
        } else {
            x = self.x * 2_i32.pow(change.abs() as u32);
            y = self.y * 2_i32.pow(change.abs() as u32);
        }

        Self {
            x,
            y,
            z: zoom_level,
            ..self
        }
    }
}

impl Zoom for PointWTime {
    fn change_zoom(self, zoom_level: i32) -> Self {
        let change = self.z - zoom_level;
        let x;
        let y;

        if change > 0 {
            x = self.point.x / 2_i32.pow(change.abs() as u32);
            y = self.point.y / 2_i32.pow(change.abs() as u32);
        } else {
            x = self.point.x * 2_i32.pow(change.abs() as u32);
            y = self.point.y * 2_i32.pow(change.abs() as u32);
        }

        Self {
            point: Point { x, y },
            z: zoom_level,
            ..self
        }
    }
}

impl std::ops::Sub for Point {
    type Output = Point;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::Output {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

pub fn draw_linestring(
    ls: &LineStringM<4326>,
    zoom_level: i32,
    sampling_zoom_level: i32,
) -> Vec<PointWTime> {
    let mut point_ext: Vec<PointWTime> = ls
        .points()
        .map(|p| {
            (
                point_to_grid((p.coord.x, p.coord.y).into(), sampling_zoom_level),
                DateTime::from_timestamp_secs(p.coord.m as i64).unwrap(),
            )
        })
        .tuple_windows()
        .map(|((ap, at), (bp, bt))| enhance_point(draw_line(ap, bp), at, bt, sampling_zoom_level))
        .flatten()
        .map(|x| x.change_zoom(zoom_level))
        .collect();

    point_ext.par_sort_by_key(|p| (p.point, p.time_start, p.time_end));
    point_ext
        .par_chunk_by(|a, b| a.point == b.point && a.time_end >= b.time_start)
        .map(|p| {
            let first = p.first().expect("Chunks are not empty");
            let last = p.last().expect("Chunks are not empty");
            PointWTime {
                time_end: last.time_end,
                ..*first
            }
        })
        .collect()
}

pub fn enhance_point(
    points: Vec<Point>,
    time_from: DateTime<Utc>,
    time_to: DateTime<Utc>,
    sampling_zoom_level: i32,
) -> Vec<PointWTime> {
    let dtime = time_to - time_from;

    let len = points.len() - 1;

    if points.len() == 1 {
        let point = points
            .first()
            .expect("It has been tested to be a single point");
        return vec![PointWTime {
            point: *point,
            z: sampling_zoom_level,
            time_start: time_from,
            time_end: time_to,
        }];
    }
    if points.len() == 0 {
        return Vec::new();
    }

    let dtime = dtime / (len as i32);

    points
        .into_iter()
        .enumerate()
        .map(|(i, p)| PointWTime {
            point: p,
            time_start: std::cmp::max(time_from, time_from + dtime * i as i32 - dtime / 2),
            time_end: std::cmp::min(
                time_to,
                time_from + dtime * i as i32 + dtime / 2 + chrono::TimeDelta::nanoseconds(1), // The one nanoseconds fix the reduce step. If it is not there, then the timestamps will not overlap and cannot be reduced. It also fixes performance which is very nice.
            ),
            z: sampling_zoom_level,
        })
        .collect()
}

/// This implementation is taken from: https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames under CC BY-SA 2.0 license
/// The only changes to the implementation is variable names such that it follows those used in the rest of the program.
#[inline(always)]
pub fn point_to_grid(point: Coord<f64>, sampling_zoom_level: i32) -> Point {
    use std::f64::consts::*;
    let n = (1 << sampling_zoom_level) as f64;

    let x = (n * (point.x + 180.0) / 360.0) as i32;
    let y_rad = point.y.to_radians();
    let y = (n * (1.0 - (y_rad.tan() + (1.0 / y_rad.cos())).ln() / PI) / 2.0) as i32;

    // let x =
    //     (1. / TAU * 2_f64.powi(sampling_zoom_level) * (PI + (point.x * PI / 180.))).floor() as i32;
    // let y = (1. / TAU
    //     * 2_f64.powi(sampling_zoom_level)
    //     * (PI - ((FRAC_PI_4 + (point.y * PI / 180.) / 2.).tan()).ln()))
    // .floor() as i32;

    Point { x, y }
}

// fn point_to_tile(point: &PointWTime, mmsi: i32, ship_data: &Arc<Ships>) -> Tile {
//     let timestamps: &[(DateTime<Utc>, DateTime<Utc>)] = point.time_stamps.as_ref();

//     let (minsog, maxsog) = timestamps
//         .into_iter()
//         .map(|x| ship_data.sog.b_tree_index.range((mmsi, x.0)..=(mmsi, x.1)))
//         .map(|x| x.into_iter().map(|x| ship_data.sog.sog[*x.1]))
//         .flatten()
//         .fold(None::<(f32, f32)>, |acc, x| match acc {
//             Some((min, max)) => Some((min.min(x), max.max(x))),
//             None => Some((x, x)),
//         })
//         .unzip();

//     let draught = timestamps
//         .into_iter()
//         .map(|x| ship_data.ship_draught.search_range_by_time(mmsi, x.0, x.1))
//         .flatten()
//         .map(|x| ship_data.ship_draught.draught[x])
//         .reduce(|acc, x| acc.max(x));

//     let (width, length) = ship_data.dimensions.search_by_key(mmsi).ok().unzip();

//     let cell_oc_time: chrono::TimeDelta = timestamps
//         .into_iter()
//         .fold(chrono::TimeDelta::nanoseconds(0), |acc, (tb, te)| {
//             acc + (*te - *tb)
//         });

//     Tile {
//         x: point.point.x,
//         y: point.point.y,
//         z: point.z,
//         max_draught: draught,
//         distinct_ship_count: 1,
//         min_sog: minsog,
//         max_sog: maxsog,
//         cell_oc_time: cell_oc_time,
//         min_length: length,
//         max_length: length,
//         min_width: width,
//         max_width: width,
//     }
// }

pub fn combine_tiles(tiles: &[Tile]) -> Option<Tile> {
    tiles
        .into_iter()
        .cloned()
        .reduce(|acc, x| combine_2_tiles(&acc, &x))
}

pub fn combine_2_tiles(a: &Tile, b: &Tile) -> Tile {
    Tile {
        max_draught: a.max_draught.map_or(b.max_draught, |a| {
            b.max_draught.map_or(Some(a), |b| Some(a.max(b)))
        }),
        distinct_ship_count: a.distinct_ship_count + b.distinct_ship_count,
        min_sog: a
            .min_sog
            .map_or(b.min_sog, |a| b.min_sog.map_or(Some(a), |b| Some(a.min(b)))),
        max_sog: a
            .max_sog
            .map_or(b.max_sog, |a| b.max_sog.map_or(Some(a), |b| Some(a.max(b)))),
        cell_oc_time: a.cell_oc_time + b.cell_oc_time,
        min_length: a.min_length.map_or(b.min_length, |a| {
            b.min_length.map_or(Some(a), |b| Some(a.min(b)))
        }),
        max_length: a.max_length.map_or(b.max_length, |a| {
            b.max_length.map_or(Some(a), |b| Some(a.max(b)))
        }),
        min_width: a.min_width.map_or(b.min_width, |a| {
            b.min_width.map_or(Some(a), |b| Some(a.min(b)))
        }),
        max_width: a.max_width.map_or(b.max_width, |a| {
            b.max_width.map_or(Some(a), |b| Some(a.max(b)))
        }),
        ..a.clone()
    }
}
pub fn point_time_duration(
    time_from: DateTime<Utc>,
    time_to: DateTime<Utc>,
    point_count: i32,
) -> chrono::TimeDelta {
    let dt = time_to.signed_duration_since(time_from);

    let duration = dt.checked_div(point_count).unwrap_or(dt);

    return duration;
}

pub fn draw_line(from: Point, to: Point) -> Vec<Point> {
    let mut coordinates: Vec<Point> = vec![];
    let dx = (to.x - from.x).abs();
    let dy = (to.y - from.y).abs();

    let sx = { if from.x < to.x { 1 } else { -1 } };
    let sy = { if from.y < to.y { 1 } else { -1 } };

    let mut error = (if dx > dy { dx } else { -dy }) / 2;
    let mut current_x = from.x;
    let mut current_y = from.y;

    loop {
        coordinates.push(Point {
            x: current_x,
            y: current_y,
        });

        if current_x == to.x && current_y == to.y {
            break;
        }

        let error2 = error;

        if error2 > -dx {
            error -= dy;
            current_x += sx;
        }
        if error2 < dy {
            error += dx;
            current_y += sy;
        }
    }

    coordinates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line() {
        let mut points: Vec<Point> = Vec::new();

        points.append(&mut draw_line(Point { x: 1, y: 1 }, Point { x: 3, y: 3 }));

        dbg!(&points);

        assert!(points[1].x == 2 && points[1].y == 2);
    }

    #[test]
    fn not_a_line() {
        let mut points: Vec<Point> = Vec::new();

        points.append(&mut draw_line(Point { x: 0, y: 0 }, Point { x: 0, y: 0 }));

        assert_eq!(points.len(), 1)
    }

    #[test]
    fn combine_tiles_same_coord() {
        let tiles = vec![
            Tile {
                x: 0,
                y: 0,
                z: 10,
                max_draught: None,
                min_sog: Some(1.0),
                max_sog: None,
                distinct_ship_count: 1,
                cell_oc_time: chrono::TimeDelta::seconds(5),
                min_length: Some(5.0),
                max_length: Some(5.0),
                min_width: None,
                max_width: Some(2.0),
            },
            Tile {
                x: 0,
                y: 0,
                z: 10,
                max_draught: Some(6.0),
                min_sog: Some(2.0),
                max_sog: Some(2.0),
                distinct_ship_count: 1,
                cell_oc_time: chrono::TimeDelta::seconds(5),
                min_length: Some(2.0),
                max_length: Some(2.0),
                min_width: None,
                max_width: Some(4.0),
            },
        ];

        let result = combine_tiles(&tiles);

        assert_eq!(
            result.unwrap(),
            Tile {
                x: 0,
                y: 0,
                z: 10,
                max_draught: Some(6.),
                min_sog: Some(1.0),
                max_sog: Some(2.0),
                distinct_ship_count: 2,
                cell_oc_time: chrono::TimeDelta::seconds(10),
                min_length: Some(2.),
                max_length: Some(5.),
                min_width: None,
                max_width: Some(4.)
            }
        );
    }

    #[test]
    fn combine_tiles_diff_ships() {
        let tiles = vec![
            Tile {
                x: 0,
                y: 0,
                z: 10,
                max_draught: None,
                min_sog: Some(1.0),
                max_sog: None,
                distinct_ship_count: 1,
                cell_oc_time: chrono::TimeDelta::seconds(5),
                min_length: Some(5.),
                max_length: Some(5.),
                min_width: None,
                max_width: Some(2.),
            },
            Tile {
                x: 0,
                y: 0,
                z: 10,
                max_draught: Some(6.),
                min_sog: Some(2.0),
                max_sog: Some(2.0),
                distinct_ship_count: 1,
                cell_oc_time: chrono::TimeDelta::seconds(5),
                min_length: Some(2.),
                max_length: Some(2.),
                min_width: None,
                max_width: Some(4.),
            },
        ];

        let result = combine_tiles(&tiles);

        assert_eq!(
            result.unwrap(),
            Tile {
                x: 0,
                y: 0,
                z: 10,
                max_draught: Some(6.),
                min_sog: Some(1.0),
                max_sog: Some(2.0),
                distinct_ship_count: 2,
                cell_oc_time: chrono::TimeDelta::seconds(10),
                min_length: Some(2.),
                max_length: Some(5.),
                min_width: None,
                max_width: Some(4.)
            }
        );
    }

    #[test]
    fn coord_to_point() {
        let cass_point = Point { x: 34586, y: 20073 }; // At zoom 16
        let cass_4326_coord = Coord::<f64> {
            x: 9.99083572,
            y: 57.01233944,
        };

        let result = point_to_grid(cass_4326_coord, 16);

        assert_eq!(cass_point, result);
    }

    #[test]
    fn test_line_drawing_with_real_world_data() {
        const HEXSTRING: &str = include_str!("../../resources/line.txt");
        let bytea = hex::decode(HEXSTRING.trim()).unwrap();
        let geom = wkb::reader::read_wkb(&bytea).unwrap();
        let line = LineStringM::<4326>::try_from(geom).unwrap();
        let result = draw_linestring(line, 5, 6);
        dbg!(&result);

        assert!(false);
    }

    // #[test]
    // fn combine_points() {
    //     let point = Point { x: 0, y: 0 };
    //     let points1 = PointWTime {
    //         point,
    //         z: 22,
    //         time_start:
    //         time_stamps: vec![
    //             (
    //                 DateTime::from_timestamp_nanos(1000),
    //                 DateTime::from_timestamp_nanos(2000),
    //             ),
    //             (
    //                 DateTime::from_timestamp_nanos(3000),
    //                 DateTime::from_timestamp_nanos(4000),
    //             ),
    //         ],
    //     };
    //     let points2 = PointWTime {
    //         point,
    //         z: 22,
    //         time_stamps: vec![
    //             (
    //                 DateTime::from_timestamp_nanos(5000),
    //                 DateTime::from_timestamp_nanos(6000),
    //             ),
    //             (
    //                 DateTime::from_timestamp_nanos(1500),
    //                 DateTime::from_timestamp_nanos(3200),
    //             ),
    //         ],
    //     };
    //     let comb = PointWTime {
    //         point,
    //         z: 22,
    //         time_stamps: vec![
    //             (
    //                 DateTime::from_timestamp_nanos(1000),
    //                 DateTime::from_timestamp_nanos(4000),
    //             ),
    //             (
    //                 DateTime::from_timestamp_nanos(5000),
    //                 DateTime::from_timestamp_nanos(6000),
    //             ),
    //         ],
    //     };

    //     let result = combine_point_with_time(&[points1, points2]).unwrap();

    //     assert_eq!(comb, result)
    // }

    // #[test]
    // fn draw_linestring_level_0_one_timestamp() {
    //     let coords = vec![
    //         CoordM::<4326> {
    //             x: 9.99077490,
    //             y: 57.01199765,
    //             m: 1759393758.,
    //         },
    //         CoordM::<4326> {
    //             x: 12.59321066,
    //             y: 55.68399700,
    //             m: 1759397358.,
    //         },
    //         CoordM::<4326> {
    //             x: 8.4437682,
    //             y: 55.4616713,
    //             m: 1759400958.,
    //         },
    //         CoordM::<4326> {
    //             x: 11.9732157,
    //             y: 57.7093381,
    //             m: 1759404558.,
    //         },
    //     ];

    //     let ls = LineStringM::new(coords).unwrap();

    //     let mut points = draw_linestring(ls.clone(), 8, 22);

    //     points.sort_by_cached_key(|a| a.point);

    //     let result: Vec<PointWTime> = points
    //         .chunk_by(|a, b| a.point == b.point)
    //         .map(|x| combine_point_with_time(x))
    //         .flatten()
    //         .collect();

    //     assert_eq!(result.len(), 9);
    // }
}
