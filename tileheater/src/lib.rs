use bincode::{Decode, Encode};
use chrono::{Datelike, Timelike};
use geo::Coord;
use geo::LineString;
use geo::Polygon;
use geo::TriangulateDelaunay;
use geo::{Distance, Geodesic};
use geo_traits::CoordTrait;
use geo_traits::GeometryTrait;
use geo_traits::LineStringTrait;
use geo_traits::PolygonTrait;
use linesonmaps::{
    algo::{
        segmenter::{segmenter, TrajectorySplit},
        stop_cluster::{cluster_to_traj_with_stop_object, DbScanConf},
    },
    types::{linestringm, multipointm::MultiPointM, pointm::PointM},
};
use pgrx::prelude::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tilerizer::tile3d::draw_triangle;
use tilerizer::Zoom;
use wkb::{
    reader::{self, Dimension},
    writer,
};

pub mod types;

::pgrx::pg_module_magic!(name, version);

#[pg_extern(parallel_safe, immutable)]
fn segment_linestring(linestring: &[u8]) -> SetOfIterator<'static, Vec<u8>> {
    let conv_wkb = reader::read_wkb(linestring).expect("Something");
    let linestringm: linestringm::LineStringM<4326> =
        linestringm::LineStringM::try_from(conv_wkb).expect("Something2");
    let func = |f, l| dist(f, l, 1000_f64) && time_dist(f, l, 60_f64);
    let splitted = segmenter(linestringm, func);

    let sub_traj = splitted.into_iter().map(|x| match x {
        TrajectorySplit::SubTrajectory(line_string_m) => {
            let mut buf: Vec<u8> = Vec::new();
            let options = wkb::writer::WriteOptions {
                endianness: wkb::Endianness::LittleEndian,
            };
            writer::write_line_string(&mut buf, &line_string_m, &options).expect("Nothing");
            buf
        }
        TrajectorySplit::Point(point_m) => {
            let mut buf: Vec<u8> = Vec::new();
            let options = wkb::writer::WriteOptions {
                endianness: wkb::Endianness::LittleEndian,
            };
            writer::write_point(&mut buf, &point_m, &options).expect("Nothing");
            buf
        }
    });
    SetOfIterator::new(sub_traj)
}

const fn time_dist(first: PointM, second: PointM, thres: f64) -> bool {
    second.coord.m - first.coord.m < thres
}

fn dist(first: PointM, second: PointM, thres: f64) -> bool {
    use geo::algorithm::line_measures::metric_spaces::Geodesic;
    Geodesic.distance(first, second) < thres
}

#[pg_extern(parallel_safe, immutable)]
fn segment_points(linestring: &[u8]) -> SetOfIterator<'static, Vec<u8>> {
    let conv_wkb = reader::read_wkb(linestring).expect("Something");
    let linestringm: linestringm::LineStringM<4326> =
        linestringm::LineStringM::try_from(conv_wkb).expect("Something2");
    let func = |f, l| dist(f, l, 1000_f64) && time_dist(f, l, 60_f64);
    let splitted = segmenter(linestringm, func);

    let sub_traj: Vec<_> = splitted
        .into_iter()
        .map(|x| match x {
            TrajectorySplit::SubTrajectory(_) => None,
            TrajectorySplit::Point(point_m) => return Some(point_m),
        })
        .flatten()
        .collect();

    let data = sub_traj.into_iter().map(|x| {
        let mut buf: Vec<u8> = Vec::new();
        let options = wkb::writer::WriteOptions {
            endianness: wkb::Endianness::LittleEndian,
        };
        writer::write_point(&mut buf, &x, &options).expect("Nothing");

        buf
    });

    SetOfIterator::new(data)
}

#[pg_extern(parallel_safe, immutable)]
fn extract_stop_objects(
    stop_objects: &[u8],
    sogs: Vec<Option<Numeric<4, 1>>>,
    min_cluster_size: i64,
    dist_thres: f64,
    time_thres: i64,
    speed_thres: f32,
) -> TableIterator<
    'static,
    (
        name!(geom, Vec<u8>),
        name!(time_start, TimestampWithTimeZone),
        name!(time_end, TimestampWithTimeZone),
    ),
> {
    let conv_wkb = reader::read_wkb(&stop_objects).expect("expected WKB");
    let multipoints = MultiPointM::try_from(conv_wkb).expect("Expected MultiPointM");

    let mut points: Vec<_> = multipoints
        .0
        .iter()
        .cloned()
        .zip(sogs.into_iter().flat_map(|x| match x {
            Some(v) => v.try_into().ok(),
            None => Some(f32::NAN),
        }))
        .collect();
    points.par_sort_by(|a, b| a.0.coord.m.total_cmp(&b.0.coord.m));

    let mut conf = DbScanConf::builder()
        .dist(|a: &PointM<4326>, b: &PointM<4326>| Geodesic.distance(*a, *b))
        .max_time_thres(chrono::TimeDelta::new(time_thres * 60, 0).expect("This did not work"))
        .speed_thres(speed_thres)
        .min_cluster_size(
            (min_cluster_size as usize)
                .try_into()
                .expect("Neither did this"),
        )
        .dist_thres(dist_thres)
        .build();
    let clusters = conf.run(&points);

    let objects = cluster_to_traj_with_stop_object(clusters)
        .0
        .into_iter()
        .flat_map(|a| match a {
            linesonmaps::algo::stop_cluster::StopOrLs::Stop { polygon, tz_tange } => {
                let mut buf: Vec<u8> = Vec::new();
                let options = wkb::writer::WriteOptions {
                    endianness: wkb::Endianness::LittleEndian,
                };
                let ts = TimestampWithTimeZone::with_timezone(
                    tz_tange.0.year() as i32,
                    tz_tange.0.month() as u8,
                    tz_tange.0.day() as u8,
                    tz_tange.0.hour() as u8,
                    tz_tange.0.minute() as u8,
                    tz_tange.0.second() as f64,
                    "Etc/UTC",
                )
                .unwrap();
                let te = TimestampWithTimeZone::with_timezone(
                    tz_tange.1.year() as i32,
                    tz_tange.1.month() as u8,
                    tz_tange.1.day() as u8,
                    tz_tange.1.hour() as u8,
                    tz_tange.1.minute() as u8,
                    tz_tange.1.second() as f64,
                    "Etc/UTC",
                )
                .unwrap();
                if polygon.exterior().0.len() < 4 {
                    return None;
                }

                wkb::writer::write_polygon(&mut buf, &polygon, &options).expect("Something else");
                Some((buf, ts, te))
            }
            _ => None,
        });
    TableIterator::new(objects)
}

#[pg_extern(parallel_safe)]
fn render_stop_object(
    poly: &[u8],
    zoom_level: i32,
    sampling_zoom_level: i32,
    filter_x: Option<i32>,
    filter_y: Option<i32>,
    filter_z: Option<i32>,
) -> TableIterator<'static, (name!(x, i32), name!(y, i32), name!(z, i32))> {
    let filter = filter_x
        .zip(filter_y.zip(filter_z))
        .map(|(x, (y, z))| (x, y, z));
    let geom = wkb::reader::read_wkb(&poly).expect("Could not read wkb");
    if geom.dimension() != Dimension::Xy {
        panic!("Received non XY dimension geometry");
    }

    let poly: Option<Polygon> = match geom.as_type() {
        geo_traits::GeometryType::Polygon(poly) => {
            let coords: Option<Vec<_>> = poly
                .exterior()
                .map(|x| x.coords().map(|c| Coord { x: c.x(), y: c.y() }).collect());
            let ls = coords.and_then(|c| Some(LineString::new(c)));
            ls.and_then(|x| Some(Polygon::new(x, vec![])))
        }
        _ => panic!("Expected Polygon"),
    };
    let points = match poly {
        Some(poly) => {
            tilerizer::tile3d::render_stop_object(&poly, zoom_level, sampling_zoom_level, filter)
        }
        None => None,
    };
    match points {
        Some(ps) => TableIterator::new(ps),
        None => TableIterator::empty(),
    }
}

#[derive(Copy, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct TimeDelta {
    pub micros: i128,
}

impl std::ops::Add<i128> for TimeDelta {
    type Output = Self;

    fn add(self, rhs: i128) -> Self::Output {
        TimeDelta {
            micros: self.micros + rhs,
        }
    }
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    #[must_use]
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
