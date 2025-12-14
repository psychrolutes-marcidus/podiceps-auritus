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
use pgrx::pg_sys::float4;
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

#[pg_extern]
fn hello_tileheater() -> &'static str {
    "Hello, tileheater"
}

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
        .max_time_thres(chrono::TimeDelta::new(30 * 60, 0).expect("This did not work"))
        .speed_thres(1.5)
        .min_cluster_size(150.try_into().expect("Neither did this"))
        .dist_thres(250.0)
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
    let triangles = poly.and_then(|p| p.constrained_triangulation(Default::default()).ok());
    let points: Option<Vec<_>> = triangles.map(|ts| {
        ts.iter()
            .map(|t| draw_triangle(*t, sampling_zoom_level))
            .flatten()
            .filter(|p| match filter {
                Some(ft) => {
                    let point = p.change_zoom(ft.2);
                    point.point.x == ft.0 && point.point.y == ft.1
                }
                None => true,
            })
            .map(|p| p.change_zoom(zoom_level))
            .map(|p| (p.point.x, p.point.y, p.z))
            .collect()
    });
    let points = points.map(|x| {
        let mut x = x;
        x.sort_by_cached_key(|x| *x);
        let points: Vec<_> = x
            .chunk_by(|a, b| a == b)
            .flat_map(|x| x.first().map(|x| x.to_owned()))
            .collect();
        points
    });
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

// #[derive(Clone, Encode, Decode, PostgresType, Deserialize, Serialize, AggregateName)]
// #[pg_binary_protocol]
// pub struct CombineTile {
//     occ_time: Option<TimeDelta>,
//     min_sog: Option<f64>,
//     max_sog: Option<f64>,
//     min_width: Option<f64>,
//     max_width: Option<f64>,
//     min_length: Option<f64>,
//     max_length: Option<f64>,
//     max_depth: Option<f64>,
//     mmsi_set: std::collections::HashSet<i64>,
// }

// #[derive(Copy, Clone, PostgresType, Serialize, Deserialize)]
// #[pg_binary_protocol]
// pub struct Tile {
//     occ_time: Option<Interval>,
//     min_sog: Option<f64>,
//     max_sog: Option<f64>,
//     min_width: Option<f64>,
//     max_width: Option<f64>,
//     min_length: Option<f64>,
//     max_length: Option<f64>,
//     max_depth: Option<f64>,
//     distinct_vesssel: i64,
// }

// #[pg_aggregate(parallel_safe)]
// impl Aggregate<CombineTile> for CombineTile {
//     type Args = (
//         name!(occ_time, Option<Interval>),
//         name!(min_sog, Option<f64>),
//         name!(max_sog, Option<f64>),
//         name!(min_width, Option<f64>),
//         name!(max_width, Option<f64>),
//         name!(min_length, Option<f64>),
//         name!(max_length, Option<f64>),
//         name!(max_depth, Option<f64>),
//         name!(mmsi, i64),
//     );
//     type Finalize = Tile;
//     const INITIAL_CONDITION: Option<&'static str> = Some(
//         r#"{"occ_time": NULL, "min_sog": NULL, "max_sog": NULL, "min_width": NULL, "max_width": NULL, "min_length": NULL, "max_length": NULL, "max_depth": NULL, "mmsi_set": {}}"#,
//     );
//     fn state(
//         mut current: Self::State,
//         arg: Self::Args,
//         _fcinfo: pg_sys::FunctionCallInfo,
//     ) -> Self::State {
//         current.occ_time = arg.0.map_or(current.occ_time, |x| {
//             current.occ_time.map_or(
//                 Some(TimeDelta {
//                     micros: x.as_micros(),
//                 }),
//                 |y| Some(y + x.as_micros()),
//             )
//         });
//         current.min_sog = min_float(current.min_sog, arg.1);
//         current.max_sog = max_float(current.max_sog, arg.2);
//         current.min_width = min_float(current.min_width, arg.3);
//         current.max_width = max_float(current.max_width, arg.4);
//         current.min_length = min_float(current.min_length, arg.5);
//         current.max_length = max_float(current.max_length, arg.6);
//         current.max_depth = max_float(current.max_depth, arg.7);
//         current.mmsi_set.insert(arg.8);
//         current
//     }
//     fn finalize(
//         current: Self::State,
//         _direct_args: Self::OrderedSetArgs,
//         _fcinfo: pg_sys::FunctionCallInfo,
//     ) -> Self::Finalize {
//         Self::Finalize {
//             distinct_vesssel: current.mmsi_set.iter().count() as i64,
//             occ_time: current
//                 .occ_time
//                 .map(|x| Interval::from_micros(x.micros as i64)),
//             min_sog: current.min_sog,
//             max_sog: current.max_sog,
//             min_width: current.min_width,
//             max_width: current.max_width,
//             min_length: current.min_length,
//             max_length: current.max_length,
//             max_depth: current.max_depth,
//         }
//     }
//     fn combine(
//         mut current: Self::State,
//         other: Self::State,
//         _fcinfo: pg_sys::FunctionCallInfo,
//     ) -> Self::State {
//         current.occ_time = current.occ_time.map_or(other.occ_time, |x| {
//             other
//                 .occ_time
//                 .map_or(current.occ_time, |y| Some(x + y.micros))
//         });
//         current.min_sog = min_float(current.min_sog, other.min_sog);
//         current.max_sog = max_float(current.max_sog, other.max_sog);
//         current.min_width = min_float(current.min_width, other.min_width);
//         current.max_width = max_float(current.max_width, other.max_width);
//         current.min_length = min_float(current.min_length, other.min_length);
//         current.max_length = max_float(current.max_length, other.max_length);
//         current.max_depth = max_float(current.max_depth, other.max_depth);
//         current.mmsi_set = current.mmsi_set.union(&other.mmsi_set).copied().collect();
//         current
//     }
// }

fn min_float(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    a.map_or(b, |x| b.map_or(Some(x), |y| Some(x.min(y))))
}
fn max_float(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    a.map_or(b, |x| b.map_or(Some(x), |y| Some(x.max(y))))
}

// #[pg_extern(parallel_safe, immutable)]
// fn extract_stop_object(
//     points: AnyArray,
// ) -> TableIterator<
//     'static,
//     (
//         name!(geom, Vec<u8>),
//         name!(time_start, TimestampWithTimeZone),
//         name!(time_end, TimestampWithTimeZone),
//     ),
// > {
//     let points: Vec<(Option<Vec<u8>>, Option<f64>)> = points
//         .into_iter()
//         .flatten()
//         .map(|x| {
//             let a = x.into::<(Option<Vec<u8>>, Option<f64>)>();

//         })
//         .collect();

// let (points, sog): (Vec<Option<Vec<u8>>>, Vec<Option<f64>>) = points.iter().map(|&x| x).unzip();

// let points: Vec<_> = points
//     .iter()
//     .flatten()
//     .map(|x| {
//         let conv_wkb = reader::read_wkb(x).expect("Something");
//         PointM::try_from(conv_wkb).expect("Something2")
//     })
//     .zip(sog.iter().map(|x| x.map(|x| x as f32)).map(|x| match x {
//         Some(s) => s,
//         None => f32::NAN,
//     }))
//     .collect();

// let mut conf = DbScanConf::builder()
//     .dist(|a: &PointM<4326>, b: &PointM<4326>| Geodesic.distance(*a, *b))
//     .max_time_thres(TimeDelta::new(30 * 60, 0).unwrap())
//     .speed_thres(1.5)
//     .min_cluster_size(10.try_into().unwrap())
//     .dist_thres(250.0)
//     .build();

// let clusters = conf.run(&points);

// let clusters: Vec<_> = cluster_to_traj_with_stop_object(clusters)
//     .0
//     .into_iter()
//     .filter_map(|x| match x {
//         linesonmaps::algo::stop_cluster::StopOrLs::Stop { polygon, tz_tange } => {
//             Some((polygon, tz_tange.0, tz_tange.1))
//         }
//         _ => None,
//     })
//     .map(|(p, ts, te)| {
//         let mut buf: Vec<u8> = Vec::new();
//         let options = wkb::writer::WriteOptions {
//             endianness: wkb::Endianness::LittleEndian,
//         };
//         writer::write_polygon(&mut buf, &p, &options).unwrap();
//         let ts = TimestampWithTimeZone::new(
//             ts.year() as i32,
//             ts.month() as u8,
//             ts.day() as u8,
//             ts.hour() as u8,
//             ts.minute() as u8,
//             ts.second() as f64,
//         )
//         .unwrap();
//         let te = TimestampWithTimeZone::new(
//             te.year() as i32,
//             te.month() as u8,
//             te.day() as u8,
//             te.hour() as u8,
//             te.minute() as u8,
//             te.second() as f64,
//         )
//         .unwrap();
//         (buf, ts, te)
//     })
//     .collect();

// TableIterator::new(clusters)
//     todo!()
// }
#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_hello_tileheater() {
        assert_eq!("Hello, tileheater", crate::hello_tileheater());
    }
}

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
