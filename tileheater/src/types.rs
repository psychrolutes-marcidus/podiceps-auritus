use std::hint::unreachable_unchecked;
use std::num::NonZero;

use chrono::{Datelike, Timelike};
use geo::Coord;
use geo::LineString;
use geo::Polygon;
use geo_traits::CoordTrait;
use geo_traits::GeometryTrait;
use geo_traits::LineStringTrait;
use geo_traits::PolygonTrait;
use linesonmaps::types::{coordm::CoordM, linem::LineM, linestringm::LineStringM, pointm::PointM};
use modeling::modeling::line_to_triangle_pair;
use pgrx::prelude::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tilerizer::{draw_linestring, point_to_grid, tile3d::draw_line_triangle, PointWTime, Zoom};
use wkb::reader::Dimension;

#[derive(Clone, Default, PostgresType, Serialize, Deserialize, AggregateName)]
pub struct RenderPointsAgg {
    points: Vec<PointWTime>,
}

#[derive(Copy, Clone, Debug)]
pub struct FilterTile(i32, i32, i32);

impl FilterTile {
    fn min_max_bb(&self, sampling_zoom_level: i32) -> (i32, i32, i32, i32) {
        let z = self.2;
        let diff = sampling_zoom_level - z;
        let x_min = self.0.saturating_pow(diff as u32);
        let y_min = self.1.saturating_pow(diff as u32);
        let x_max = (self.0 + 1).saturating_pow(diff as u32) - 1;
        let y_max = (self.1 + 1).saturating_pow(diff as u32) - 1;

        (x_min, y_min, x_max, y_max)
    }
}

#[pg_extern(parallel_safe, immutable)]
fn render_geom(
    geom: &[u8],
    zoom_level: i32,
    sampling_zoom_level: i32,
    a: Option<i16>,
    b: Option<i16>,
    c: Option<i16>,
    d: Option<i16>,
    filter_tile_x: Option<i32>,
    filter_tile_y: Option<i32>,
    filter_tile_z: Option<i32>,
) -> TableIterator<
    'static,
    (
        name!(x, i32),
        name!(y, i32),
        name!(z, i32),
        name!(time_start, TimestampWithTimeZone),
        name!(time_end, TimestampWithTimeZone),
    ),
> {
    let filter_tile = filter_tile_x
        .zip(filter_tile_y.zip(filter_tile_z))
        .map(|(x, (y, z))| FilterTile(x, y, z));

    let geom = wkb::reader::read_wkb(&geom).expect("Could not read wkb");
    if geom.dimension() != Dimension::Xym {
        panic!("Received non XYM dimension geometry. It will be ignored");
    }

    let points: Vec<_> = match geom.geometry_type() {
        wkb::reader::GeometryType::Point => {
            let pointm: PointM<4326> =
                PointM::try_from(CoordM::try_from(geom).expect("Expected a PointM"))
                    .expect("Expected a PointM");
            vec![render_point(pointm, sampling_zoom_level).map(|x| x.change_zoom(zoom_level))]
                .into_iter()
                .flatten()
                .collect()
        }
        wkb::reader::GeometryType::LineString => {
            let linem: LineStringM<4326> =
                LineStringM::try_from(geom).expect("Expected a LinestringM");
            let length = linem.0.len();
            let linem = &linem;
            let linem: Vec<LineStringM<4326>> = filter_tile.map_or(vec![linem.to_owned()], |ft| {
                let indexes: Vec<usize> = linem
                    .0
                    .windows(2)
                    .enumerate()
                    .filter(|(_, s)| match s {
                        [a, b] => {
                            let a_grid = point_to_grid((a.x, a.y).into(), ft.2);
                            let b_grid = point_to_grid((b.x, b.y).into(), ft.2);
                            let x_min = a_grid.x.min(b_grid.x);
                            let y_min = a_grid.y.min(b_grid.y);
                            let x_max = a_grid.x.max(b_grid.x);
                            let y_max = a_grid.y.max(b_grid.y);
                            x_min <= ft.0 && x_max >= ft.0 && y_min <= ft.1 && y_max >= ft.1
                        }
                        _ => unsafe { unreachable_unchecked() },
                    })
                    .map(|(i, _)| i)
                    .collect();
                indexes
                    .chunk_by(|a, b| *a == b - 1)
                    .map(|x| {
                        let first = x.first().copied();
                        let last = x
                            .last()
                            .map(|x| std::cmp::max(x.saturating_add(1), length - 1));
                        first.unwrap()..last.unwrap()
                    })
                    .map(|x| LineStringM::try_from(linem.0[x].to_vec()))
                    .flatten()
                    .collect()
            });
            let values = a.zip(b.zip(c.zip(d))).map(|(a, (b, (c, d)))| (a, b, c, d));
            match values {
                Some((a, b, c, d)) => {
                    let mut points: Vec<_> = linem
                        .iter()
                        .map(|lm| {
                            lm.lines()
                                .map(|line: LineM<4326>| {
                                    line_to_triangle_pair(
                                        &line, a as f64, b as f64, c as f64, d as f64,
                                    )
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
                None => linem
                    .iter()
                    .map(|lm| draw_linestring(lm, zoom_level, sampling_zoom_level))
                    .flatten()
                    .filter(|p| match filter_tile {
                        Some(ft) => {
                            let point = p.change_zoom(ft.2);
                            point.point.x == ft.0 && point.point.y == ft.1
                        }
                        None => true,
                    })
                    .collect(),
            }
        }
        wkb::reader::GeometryType::Polygon => todo!(),
        wkb::reader::GeometryType::MultiPoint => todo!(),
        wkb::reader::GeometryType::MultiLineString => todo!(),
        wkb::reader::GeometryType::MultiPolygon => todo!(),
        wkb::reader::GeometryType::GeometryCollection => todo!(),
        _ => todo!(),
    };
    let points: Vec<_> = points
        .into_iter()
        .map(|p| {
            (
                p.point.x,
                p.point.y,
                p.z,
                TimestampWithTimeZone::with_timezone(
                    p.time_start.year() as i32,
                    p.time_start.month() as u8,
                    p.time_start.day() as u8,
                    p.time_start.hour() as u8,
                    p.time_start.minute() as u8,
                    p.time_start.second() as f64
                        + (p.time_start.nanosecond() as f64 / 1000000000.0),
                    "Etc/UTC",
                )
                .unwrap(),
                TimestampWithTimeZone::with_timezone(
                    p.time_end.year() as i32,
                    p.time_end.month() as u8,
                    p.time_end.day() as u8,
                    p.time_end.hour() as u8,
                    p.time_end.minute() as u8,
                    p.time_end.second() as f64 + (p.time_end.nanosecond() as f64 / 1000000000.0),
                    "Etc/UTC",
                )
                .unwrap(),
            )
        })
        .collect();
    TableIterator::new(points)
}

fn render_point(point: PointM, sampling_zoom_level: i32) -> Option<PointWTime> {
    let grid_point = point_to_grid((point.coord.x, point.coord.y).into(), sampling_zoom_level);
    Some(PointWTime {
        point: grid_point,
        z: sampling_zoom_level,
        time_start: chrono::DateTime::from_timestamp_secs(point.coord.m as i64)
            .expect("Should work"),
        time_end: chrono::DateTime::from_timestamp_secs(point.coord.m as i64).expect("Should work"),
    })
}
