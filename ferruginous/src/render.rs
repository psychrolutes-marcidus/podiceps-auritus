use std::error::Error;

use chrono::DateTime;
use duckdb::{
    Connection,
    core::{Inserter, LogicalTypeHandle, LogicalTypeId},
    vscalar::{ScalarFunctionSignature, VScalar},
};
use fafo::{
    ErrorMeasurementConf, cells_relative_coverage_by_polygon,
    line_error_relative_to_perfect_and_centroid, triangle_pair_to_polygon,
    util::ground_truth_to_cell_centroid_geodesic, xyzcell::Cell,
};
use linesonmaps::types::{coordm::CoordM, linem::LineM, pointm::PointM};
use modeling::modeling::{LineTriangle, line_to_triangle_pair};
use tilerizer::{
    PointWTime, Zoom, draw_line, enhance_point, point_to_grid, tile3d::draw_line_triangle,
};
use wkb::writer;

pub fn extension_entrypoint(con: &Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<RenderGeom>("render_geom")?;
    con.register_scalar_function::<Polyganize>("polyganize")?;
    Ok(())
}

#[derive(Debug)]
enum RenderMethod {
    Polygon(LineTriangle<4326>, LineTriangle<4326>),
    Line(PointM<4326>, PointM<4326>),
    Point(PointM<4326>),
}

struct RenderGeom;

impl VScalar for RenderGeom {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // This is data preparation.
        // In order to use the data in Rust it must first be transformed into Rust slices.
        let input_len = input.len();
        let from_point = input.struct_vector(0);
        let to_point = input.struct_vector(1);
        let dimensions = input.struct_vector(2);
        let metadata = input.struct_vector(3);
        let scoring = input.struct_vector(4);
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
        let to_lon_nulls = (0..input_len)
            .into_iter()
            .map(|x| to_lon.row_is_null(x as u64));
        let to_lat_nulls = (0..input_len)
            .into_iter()
            .map(|x| to_lat.row_is_null(x as u64));
        let to_time_nulls = (0..input_len)
            .into_iter()
            .map(|x| to_time.row_is_null(x as u64));
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
        let to_bow_nulls = (0..input_len)
            .into_iter()
            .map(|x| to_bow.row_is_null(x as u64));
        let to_bow_s: &[f32] = to_bow.as_slice_with_len(input_len);
        let to_bow_option = to_bow_s.iter().zip(to_bow_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });
        let to_starboard = dimensions.child(1, input_len);
        let to_starboard_nulls = (0..input_len)
            .into_iter()
            .map(|x| to_starboard.row_is_null(x as u64));
        let to_starboard_s: &[f32] = to_starboard.as_slice_with_len(input_len);
        let to_starboard_option = to_starboard_s
            .iter()
            .zip(to_starboard_nulls)
            .map(|(&d, n)| match n {
                false => Some(d),
                true => None,
            });
        let to_stern = dimensions.child(2, input_len);
        let to_stern_nulls = (0..input_len)
            .into_iter()
            .map(|x| to_stern.row_is_null(x as u64));
        let to_stern_s: &[f32] = to_stern.as_slice_with_len(input_len);
        let to_stern_option = to_stern_s
            .iter()
            .zip(to_stern_nulls)
            .map(|(&d, n)| match n {
                false => Some(d),
                true => None,
            });
        let to_port = dimensions.child(3, input_len);
        let to_port_nulls = (0..input_len)
            .into_iter()
            .map(|x| to_port.row_is_null(x as u64));
        let to_port_s: &[f32] = to_port.as_slice_with_len(input_len);
        let to_port_option = to_port_s.iter().zip(to_port_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });

        let dimensions = to_bow_option
            .zip(to_starboard_option)
            .zip(to_stern_option)
            .zip(to_port_option)
            .map(|(((a, b), c), d)| (a, b, c, d))
            .map(|(a, b, c, d)| a.zip(b).zip(c).zip(d).map(|(((a, b), c), d)| (a, b, c, d)));
        let x = metadata.child(0, input_len);
        let y = metadata.child(1, input_len);
        let level = metadata.child(2, input_len);
        let x_s: &[u32] = x.as_slice_with_len(input_len);
        let y_s: &[u32] = y.as_slice_with_len(input_len);
        let level_s: &[u8] = level.as_slice_with_len(input_len);

        let from_point = from_lon_s
            .iter()
            .zip(from_lat_s.iter())
            .zip(from_time_s.iter())
            .map(|((lon, lat), time)| (lon, lat, time))
            .map(|(&lat, &lon, &t)| {
                (
                    CoordM::<4326> {
                        x: lat as f64,
                        y: lon as f64,
                        m: t,
                    },
                    t,
                )
            });
        let to_point = to_lon_option
            .zip(to_lat_option)
            .zip(to_time_option)
            .map(|((lat, lon), time)| (lat, lon, time))
            .map(|(lat, lon, t)| {
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

        // The scoring_parameters

        let draught_dist_mmsi = scoring.child(0, input_len);
        let draught_dist_type = scoring.child(1, input_len);
        let draughts_null = scoring.child(2, input_len);
        let r_squared = scoring.child(3, input_len);
        let draught_dist_mmsi_s: &[f32] = draught_dist_mmsi.as_slice_with_len(input_len);
        let draught_dist_type_s: &[f32] = draught_dist_type.as_slice_with_len(input_len);
        let draughts_null_s: &[f32] = draughts_null.as_slice_with_len(input_len);
        let r_squared_s: &[f32] = r_squared.as_slice_with_len(input_len);

        let scoring_vals = draught_dist_mmsi_s
            .iter()
            .zip(draught_dist_type_s.iter())
            .zip(draughts_null_s.iter())
            .zip(r_squared_s.iter())
            .enumerate()
            .map(|(i, (((d_mmsi, d_type), nulls), r_sq))| {
                if draught_dist_mmsi.row_is_null(i as u64)
                    || draught_dist_type.row_is_null(i as u64)
                    || draughts_null.row_is_null(i as u64)
                    || r_squared.row_is_null(i as u64)
                {
                    return None;
                }
                return Some((d_mmsi, d_type, nulls, r_sq));
            });

        let weights = algorithms::cell::judweight_vessel();
        // Here the actual computation starts

        let data = from_point
            .zip(to_point)
            .zip(dimensions)
            .zip(
                level_s
                    .iter()
                    .map(|&x| x as i32)
                    .zip(x_s.iter())
                    .zip(y_s.iter())
                    .map(|((z, &x), &y)| (x as i32, y as i32, z as u32)),
            )
            .map(|(((from, to), dim), s)| (from, to, dim, s))
            .map(|(from_point, to_point, dim, cell)| {
                let new_cell = Cell::from(cell);
                let cell_iter = || std::iter::repeat_n(new_cell, 1);
                let conf = ErrorMeasurementConf::builder()
                    .method(fafo::ErrorMeasurementMethod::Geodesic)
                    .zoom(cell.2 as u8)
                    .build();
                if let Some(to_point) = to_point {
                    let dist = conf
                        .cell_distance_to_ground_truth(
                            (from_point.0.into(), to_point.0.into()),
                            cell_iter(),
                        )
                        .iter()
                        .map(|x| x.1)
                        .last()
                        .unwrap_or_default();
                    let line = LineM::from((from_point.0, to_point.0));
                    if let Some(dim) = dim {
                        let (tri1, tri2) = line_to_triangle_pair(
                            &line,
                            dim.0 as f64,
                            dim.1 as f64,
                            dim.2 as f64,
                            dim.3 as f64,
                        );
                        let cov = cells_relative_coverage_by_polygon((&tri1, &tri2), cell_iter())
                            .last()
                            .map(|x| x.1)
                            .unwrap_or_default();
                        return (cov, dist);
                    }
                    let cov = line_error_relative_to_perfect_and_centroid(
                        (from_point.0.into(), to_point.0.into()),
                        cell_iter(),
                    )
                    .iter()
                    .map(|x| x.1)
                    .last()
                    .unwrap_or_default();
                    return (cov, dist);
                }
                let dist =
                    ground_truth_to_cell_centroid_geodesic(PointM::from(from_point.0), &new_cell);
                return (0., dist);
            })
            .map(|x| (x.0, (1. - x.1 / 500.).clamp(0., 1.)))
            .zip(scoring_vals)
            .map(|(cells, score_p)| {
                score_p.inspect(|x| {
                    if *x.0 > 1.0 || *x.1 > 1.0 || *x.2 > 1.0 || *x.3 > 1.0 {
                        dbg!(&x);
                    }
                });

                if cells.0 > 1.0 || cells.1 > 1.0 {
                    dbg!(&cells);
                }
                match score_p {
                    Some(s) => mul_arr_sum(
                        weights,
                        [*s.0, *s.1, *s.2, *s.3, cells.0 as f32, cells.1 as f32],
                    ),
                    None => 0.,
                }
            });
        let score: Vec<_> = data.collect();
        // let mut x_out = struct_out.child(0, x.len());
        // let mut y_out = struct_out.child(1, y.len());
        // let mut z_out = struct_out.child(2, z.len());
        // let mut time_begin_out = struct_out.child(0, tb.len());
        // let mut time_end_out = struct_out.child(1, te.len());
        let mut score_out = output.flat_vector();
        // x_out.copy(&x);
        // y_out.copy(&y);
        // z_out.copy(&z);
        // time_begin_out.copy(&tb);
        // time_end_out.copy(&te);
        score_out.copy(&score);

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
            ("x", LogicalTypeHandle::from(LogicalTypeId::UInteger)),
            ("y", LogicalTypeHandle::from(LogicalTypeId::UInteger)),
            ("level", LogicalTypeHandle::from(LogicalTypeId::UTinyint)),
        ];
        let scoring_params = [
            (
                "draught_dist_mmsi",
                LogicalTypeHandle::from(LogicalTypeId::Float),
            ),
            (
                "draught_dist_type",
                LogicalTypeHandle::from(LogicalTypeId::Float),
            ),
            (
                "draughts_null",
                LogicalTypeHandle::from(LogicalTypeId::Float),
            ),
            ("r_squared", LogicalTypeHandle::from(LogicalTypeId::Float)),
        ];
        let params = vec![
            LogicalTypeHandle::struct_type(&point),
            LogicalTypeHandle::struct_type(&point),
            LogicalTypeHandle::struct_type(&dimensions),
            LogicalTypeHandle::struct_type(&metadata),
            LogicalTypeHandle::struct_type(&scoring_params),
        ];
        let output = LogicalTypeHandle::from(LogicalTypeId::Float);
        vec![ScalarFunctionSignature::exact(params, output)]
    }
}

fn mul_arr_sum<const N: usize>(a: [f32; N], b: [f32; N]) -> f32 {
    a.iter().zip(b.iter()).map(|(&a, &b)| a * b).sum()
}

struct Polyganize;

impl VScalar for Polyganize {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input_len = input.len();
        let from_point = input.struct_vector(0);
        let to_point_input = input.struct_vector(1);
        let dimensions_input = input.struct_vector(2);

        let from_lon = from_point.child(0, input_len);
        let from_lat = from_point.child(1, input_len);
        let from_time = from_point.child(2, input_len);
        let from_lon_s: &[f32] = from_lon.as_slice_with_len(input_len);
        let from_lat_s: &[f32] = from_lat.as_slice_with_len(input_len);
        let from_time_s: &[f64] = from_time.as_slice_with_len(input_len);
        let to_lon = to_point_input.child(0, input_len);
        let to_lat = to_point_input.child(1, input_len);
        let to_time = to_point_input.child(2, input_len);
        let to_lon_s: &[f32] = to_lon.as_slice_with_len(input_len);
        let to_lat_s: &[f32] = to_lat.as_slice_with_len(input_len);
        let to_time_s: &[f64] = to_time.as_slice_with_len(input_len);

        let to_bow = dimensions_input.child(0, input_len);
        let to_starboard = dimensions_input.child(1, input_len);
        let to_stern = dimensions_input.child(2, input_len);
        let to_port = dimensions_input.child(3, input_len);

        let to_bow_s: &[f32] = to_bow.as_slice_with_len(input_len);
        let to_starboard_s: &[f32] = to_starboard.as_slice_with_len(input_len);
        let to_stern_s: &[f32] = to_stern.as_slice_with_len(input_len);
        let to_port_s: &[f32] = to_port.as_slice_with_len(input_len);

        let from_point = point_zipper(from_lon_s, from_lat_s, from_time_s);
        let to_point = point_zipper(to_lon_s, to_lat_s, to_time_s);

        let dimensions = to_bow_s
            .iter()
            .zip(to_starboard_s.iter())
            .zip(to_stern_s.iter())
            .zip(to_port_s.iter())
            .map(|(((&b, &sa), &se), &p)| (b as f64, sa as f64, se as f64, p as f64));

        let mut flat_out = output.flat_vector();
        let is_null = |x: u64| {
            to_lon.row_is_null(x)
                || to_lat.row_is_null(x)
                || to_time.row_is_null(x)
                || to_bow.row_is_null(x)
                || to_starboard.row_is_null(x)
                || to_stern.row_is_null(x)
                || to_port.row_is_null(x)
        };
        from_point
            .zip(to_point)
            .zip(dimensions)
            .map(|((f, t), d)| (f, t, d))
            .enumerate()
            .map(|(i, (f, t, d))| {
                if is_null(i as u64) {
                    return vec![];
                }
                let line: LineM<4326> = LineM::from((f, t));
                let (tri1, tri2) = line_to_triangle_pair(&line, d.0, d.1, d.2, d.3);
                let poly = triangle_pair_to_polygon((&tri1, &tri2));
                let mut buf: Vec<u8> = Vec::new();
                let options = wkb::writer::WriteOptions {
                    endianness: wkb::Endianness::LittleEndian,
                };
                writer::write_polygon(&mut buf, &poly, &options)
                    .expect("Could not write polygon to WKB");
                buf
            })
            .enumerate()
            .for_each(|(i, v)| {
                if is_null(i as u64) {
                    flat_out.set_null(i);
                }
                flat_out.insert(i, &v);
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
        let params = vec![
            LogicalTypeHandle::struct_type(&point),
            LogicalTypeHandle::struct_type(&point),
            LogicalTypeHandle::struct_type(&dimensions),
        ];
        let output = LogicalTypeHandle::from(LogicalTypeId::Blob);
        vec![ScalarFunctionSignature::exact(params, output)]
    }
}

fn point_zipper(lon: &[f32], lat: &[f32], time: &[f64]) -> impl Iterator<Item = CoordM<4326>> {
    lon.iter()
        .zip(lat.iter())
        .zip(time.iter())
        .map(|((&lon, &lat), &time)| CoordM::<4326> {
            x: lon as f64,
            y: lat as f64,
            m: time,
        })
}
