use std::error::Error;

use chrono::DateTime;
use duckdb::{
    Connection,
    core::{LogicalTypeHandle, LogicalTypeId},
    vscalar::{ScalarFunctionSignature, VScalar},
};
use itertools::{Itertools, izip};
use linesonmaps::types::{coordm::CoordM, linem::LineM};
use modeling::modeling::line_to_triangle_pair;
use tilerizer::{
    Point, PointWTime, Zoom, draw_line, enhance_point, point_to_grid, tile3d::draw_line_triangle,
};

use rust_decimal::prelude::*;

pub fn extension_entrypoint(con: &Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<RenderGeom>("render_geom")?;
    Ok(())
}

struct RenderGeom;

impl VScalar for RenderGeom {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input_len = input.len();
        let from_point = input.struct_vector(0);
        let to_point = input.struct_vector(1);
        let dimensions = input.struct_vector(2);
        let metadata = input.struct_vector(3);
        let from_lon = from_point.child(0, input_len);
        let from_lat = from_point.child(1, input_len);
        let from_time = from_point.child(2, input_len);
        let from_lon_s: &[f32] = from_lon.as_slice_with_len(input_len);
        let from_lat_s: &[f32] = from_lat.as_slice_with_len(input_len);
        let from_time_s: &[f64] = from_time.as_slice_with_len(input_len);
        let to_lon = to_point.child(0, input_len);
        let to_lat = to_point.child(1, input_len);
        let to_time = to_point.child(2, input_len);
        let to_lon_s: &[f32] = to_lon.as_slice_with_len(input_len);
        let to_lat_s: &[f32] = to_lat.as_slice_with_len(input_len);
        let to_time_s: &[f64] = to_time.as_slice_with_len(input_len);
        let to_lon_nulls = (0..input_len).map(|x| to_lon.row_is_null(x as u64));
        let to_lat_nulls = (0..input_len).map(|x| to_lat.row_is_null(x as u64));
        let to_time_nulls = (0..input_len).map(|x| to_time.row_is_null(x as u64));
        let to_lon_option = to_lon_s.iter().zip(to_lon_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });
        let to_lat_option = to_lat_s.iter().zip(to_lat_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });

        let to_time_option = to_time_s.iter().zip(to_time_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });
        let to_bow = dimensions.child(0, input_len);
        let to_bow_nulls = (0..input_len).map(|x| to_bow.row_is_null(x as u64));
        let to_bow_s: &[f32] = to_bow.as_slice_with_len(input_len);
        let to_bow_option = to_bow_s.iter().zip(to_bow_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });
        let to_starboard = dimensions.child(1, input_len);
        let to_starboard_nulls = (0..input_len).map(|x| to_starboard.row_is_null(x as u64));
        let to_starboard_s: &[f32] = to_starboard.as_slice_with_len(input_len);
        let to_starboard_option = to_starboard_s
            .iter()
            .zip(to_starboard_nulls)
            .map(|(&d, n)| match n {
                false => Some(d),
                true => None,
            });
        let to_stern = dimensions.child(2, input_len);
        let to_stern_nulls = (0..input_len).map(|x| to_stern.row_is_null(x as u64));
        let to_stern_s: &[f32] = to_stern.as_slice_with_len(input_len);
        let to_stern_option = to_stern_s
            .iter()
            .zip(to_stern_nulls)
            .map(|(&d, n)| match n {
                false => Some(d),
                true => None,
            });
        let to_port = dimensions.child(3, input_len);
        let to_port_nulls = (0..input_len).map(|x| to_port.row_is_null(x as u64));
        let to_port_s: &[f32] = to_port.as_slice_with_len(input_len);
        let to_port_option = to_port_s.iter().zip(to_port_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });

        let dimensions = izip!(
            to_bow_option,
            to_starboard_option,
            to_stern_option,
            to_port_option
        )
        .map(|(a, b, c, d)| a.zip(b).zip(c).zip(d).map(|(((a, b), c), d)| (a, b, c, d)));
        let level = metadata.child(0, input_len);
        let sample_level = metadata.child(1, input_len);
        let level_s: &[u8] = level.as_slice_with_len(input_len);
        let sample_level_s: &[u8] = sample_level.as_slice_with_len(input_len);
        let from_point = izip!(from_lon_s.iter(), from_lat_s.iter(), from_time_s.iter(),).map(
            |(&lat, &lon, &t)| {
                (
                    CoordM::<4326> {
                        x: lat as f64,
                        y: lon as f64,
                        m: t,
                    },
                    t,
                )
            },
        );
        let to_point = izip!(to_lon_option, to_lat_option, to_time_option,).map(|(lat, lon, t)| {
            lat.zip(lon).zip(t).map(|((x, y), t)| {
                (
                    CoordM::<4326> {
                        x: x as f64,
                        y: y as f64,
                        m: t,
                    },
                    t,
                )
            })
        });

        let mut lengths: Vec<usize> = Vec::with_capacity(input_len);

        let data = izip!(
            from_point,
            to_point,
            dimensions,
            sample_level_s.iter().map(|&x| x as i32)
        )
        // .inspect(|_| println!("Entering rendering"))
        .map(|(from_point, to_point, dim, sam_lev)| {
            // Convert point to the grid.
            let from_point_grid = point_to_grid((from_point.0.x, from_point.0.y).into(), sam_lev);
            // Check if there is a point that a line should go to.
            if let Some(to_point) = to_point {
                // Convert the other point.
                let to_point_grid = point_to_grid((to_point.0.x, to_point.0.y).into(), sam_lev);
                // Check if the vessel has any dimensions.
                if let Some(dim) = dim {
                    let line: LineM<4326> = LineM::from((from_point.0, to_point.0));
                    let (tri1, tri2) = line_to_triangle_pair(
                        &line,
                        dim.0 as f64,
                        dim.1 as f64,
                        dim.2 as f64,
                        dim.3 as f64,
                    );
                    let mut points = draw_line_triangle(tri1, sam_lev);
                    let points2 = draw_line_triangle(tri2, sam_lev);
                    points.extend_from_slice(&points2);
                    return points;
                }
                return enhance_point(
                    draw_line(from_point_grid, to_point_grid),
                    DateTime::from_timestamp_secs(from_point.1 as i64).unwrap(),
                    DateTime::from_timestamp_secs(to_point.1 as i64).unwrap(),
                    sam_lev,
                );
            }
            return vec![PointWTime {
                point: from_point_grid,
                z: sam_lev,
                time_start: DateTime::from_timestamp_secs(from_point.1 as i64).unwrap(),
                time_end: DateTime::from_timestamp_secs(from_point.1 as i64).unwrap(),
            }];
        })
        // .inspect(|_| println!("Finished rendering"))
        .zip(level_s.iter().map(|&x| x as i32))
        // .inspect(|_| println!("Entering reduction step"))
        .map(|(d, s)| {
            let mut points = d.iter().map(|x| x.change_zoom(s)).collect::<Vec<_>>();
            points.sort_by_cached_key(|x| (x.point, x.time_start, x.time_end));
            points
                .chunk_by(|a, b| a.point == b.point && a.time_end >= b.time_start)
                .map(|x| {
                    let first = x.first().expect("Chunks are not empty");
                    let last = x.last().expect("Chunks are not empty");
                    PointWTime {
                        time_end: last.time_end,
                        ..*first
                    }
                })
                .collect::<Vec<_>>()
        })
        .inspect(|x| {
            lengths.push(x.len());
            // println!(
            //     "Current Capacity: {}",
            //     lengths.iter().cloned().sum::<usize>()
            // )
        });
        let (x, y, z, tb, te): (Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>) = data
            .flatten()
            // .inspect(|_| println!("Entering map to tuple"))
            .map(
                |PointWTime {
                     point,
                     z,
                     time_start,
                     time_end,
                 }| {
                    (
                        point.x,
                        point.y,
                        z,
                        time_start.timestamp_millis() as f64 / 1000.,
                        time_end.timestamp_millis() as f64 / 1000.,
                    )
                },
            )
            // .inspect(|_| println!("Finish tuple map, entering multiunzip"))
            .multiunzip();
        let mut list_out = output.list_vector();
        let struct_out = list_out.struct_child(lengths.iter().sum());
        let mut x_out = struct_out.child(0, x.len());
        let mut y_out = struct_out.child(1, y.len());
        let mut z_out = struct_out.child(2, z.len());
        let mut time_begin_out = struct_out.child(3, tb.len());
        let mut time_end_out = struct_out.child(4, te.len());
        x_out.copy(&x);
        y_out.copy(&y);
        z_out.copy(&z);
        time_begin_out.copy(&tb);
        time_end_out.copy(&te);
        lengths.iter().fold((0, 0), |acc, &x| {
            list_out.set_entry(acc.0, acc.1, x);
            (acc.0 + 1, acc.1 + x)
        });
        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        let point = [
            ("lon", LogicalTypeHandle::from(LogicalTypeId::Float)),
            ("lat", LogicalTypeHandle::from(LogicalTypeId::Float)),
            ("time", LogicalTypeHandle::from(LogicalTypeId::Double)),
        ];
        let dimensions = [
            ("to_bow", LogicalTypeHandle::from(LogicalTypeId::Float)),
            (
                "to_starboard",
                LogicalTypeHandle::from(LogicalTypeId::Float),
            ),
            ("to_stern", LogicalTypeHandle::from(LogicalTypeId::Float)),
            ("to_port", LogicalTypeHandle::from(LogicalTypeId::Float)),
        ];
        let metadata = [
            ("level", LogicalTypeHandle::from(LogicalTypeId::UTinyint)),
            (
                "sample_level",
                LogicalTypeHandle::from(LogicalTypeId::UTinyint),
            ),
        ];
        let output_data = [
            ("x", LogicalTypeHandle::from(LogicalTypeId::Integer)),
            ("y", LogicalTypeHandle::from(LogicalTypeId::Integer)),
            ("z", LogicalTypeHandle::from(LogicalTypeId::Integer)),
            ("time_start", LogicalTypeHandle::from(LogicalTypeId::Double)),
            ("time_end", LogicalTypeHandle::from(LogicalTypeId::Double)),
            // ("d_to_ais", LogicalTypeHandle::from(LogicalTypeId::Float)),
            // (
            //     "cell_covered",
            //     LogicalTypeHandle::from(LogicalTypeId::Float),
            // ),
        ];

        let params = vec![
            LogicalTypeHandle::struct_type(&point),
            LogicalTypeHandle::struct_type(&point),
            LogicalTypeHandle::struct_type(&dimensions),
            LogicalTypeHandle::struct_type(&metadata),
        ];
        let output = LogicalTypeHandle::list(&LogicalTypeHandle::struct_type(&output_data));
        vec![ScalarFunctionSignature::exact(params, output)]
    }
}
