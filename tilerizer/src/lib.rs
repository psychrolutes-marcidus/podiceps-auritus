use chrono::prelude::*;
use geo_types::Coord;
use itertools::Itertools;
use linesonmaps::types::{linem::LineM, linestringm::LineStringM};
use modeling::modeling::line_to_triangle_pair;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::tile3d::draw_line_triangle;

pub mod tile3d;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
pub struct PointWZ {
    pub point: Point,
    pub z: i32,
}
#[derive(Copy, Clone, Debug)]
pub struct FilterTile(pub i32, pub i32, pub i32);

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

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct PointWTime {
    pub point: Point,
    pub z: i32,
    pub time_start: DateTime<Utc>,
    pub time_end: DateTime<Utc>,
}

pub trait Zoom {
    fn change_zoom(self, zoom_level: i32) -> Self;
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
    ls: &[LineStringM<4326>],
    zoom_level: i32,
    sampling_zoom_level: i32,
    filter_tile: Option<FilterTile>,
) -> Vec<PointWTime> {
    ls.iter()
        .map(|ls| {
            let mut point_ext: Vec<PointWTime> = ls
                .points()
                .map(|p| {
                    (
                        point_to_grid((p.coord.x, p.coord.y).into(), sampling_zoom_level),
                        DateTime::from_timestamp_secs(p.coord.m as i64).unwrap(),
                    )
                })
                .tuple_windows()
                .map(|((ap, at), (bp, bt))| {
                    enhance_point(draw_line(ap, bp), at, bt, sampling_zoom_level)
                })
                .flatten()
                .filter(|p| match filter_tile {
                    Some(ft) => {
                        let point = p.change_zoom(ft.2);
                        point.point.x == ft.0 && point.point.y == ft.1
                    }
                    None => true,
                })
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
                .collect::<Vec<PointWTime>>()
        })
        .flatten()
        .collect()
}

pub fn draw_2d_vessel(
    ls: &[LineStringM<4326>],
    a: i16,
    b: i16,
    c: i16,
    d: i16,
    zoom_level: i32,
    sampling_zoom_level: i32,
    filter_tile: Option<FilterTile>,
) -> Vec<PointWTime> {
    let mut points: Vec<_> = ls
        .iter()
        .map(|lm| {
            lm.lines()
                .map(|line: LineM<4326>| {
                    line_to_triangle_pair(&line, a as f64, b as f64, c as f64, d as f64)
                })
                .flat_map(|(tri1, tri2)| {
                    [
                        draw_line_triangle(tri1, sampling_zoom_level),
                        draw_line_triangle(tri2, sampling_zoom_level),
                    ]
                })
                .flatten()
                .map(|x| x.change_zoom(zoom_level))
                .collect::<Vec<_>>()
        })
        .flatten()
        .filter(|p: &PointWTime| match filter_tile {
            Some(ft) => {
                let point = p.change_zoom(ft.2);
                point.point.x == ft.0 && point.point.y == ft.1
            }
            None => true,
        })
        .collect();
    points.par_sort_by_key(|p| (p.point, p.time_start, p.time_end));
    points
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
    fn coord_to_point() {
        let cass_point = Point { x: 34586, y: 20073 }; // At zoom 16
        let cass_4326_coord = Coord::<f64> {
            x: 9.99083572,
            y: 57.01233944,
        };

        let result = point_to_grid(cass_4326_coord, 16);

        assert_eq!(cass_point, result);
    }
}
