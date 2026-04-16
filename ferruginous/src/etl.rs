use algorithms::cell::judweight_depth;
use algorithms::cell::relative_to_bounds;
use algorithms::cell::MinmaxBounds;
use geo::Distance;
use itertools::izip;
use linesonmaps::algo::segmenter::segment_timestamp;
use linesonmaps::types::coordm::CoordM;
use linesonmaps::types::pointm::PointM;
use std::error::Error;

use duckdb::{
    core::{LogicalTypeHandle, LogicalTypeId},
    vscalar::{ScalarFunctionSignature, VScalar},
    Connection,
};

pub fn extension_entrypoint(con: &Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<DDMReliability>("ddm_reliability")?;
    con.register_scalar_function::<ExtractTrajectories>("trajectory_split")?;

    Ok(())
}

struct DDMReliability;

impl VScalar for DDMReliability {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = input.flat_vector(0);
        let year = input.flat_vector(1);

        let nullables = (0..input.len()).map(|x| year.row_is_null(x as u64));

        let slice_year: &[u32] = year.as_slice_with_len(input.len());
        let slice_year = slice_year.iter().zip(nullables).map(|(&v, n)| match n {
            true => 0,
            false => v,
        });

        let slice_source: &[u8] = source.as_slice_with_len(input.len());

        let age_bounds = MinmaxBounds {
            min: 2000.,
            max: 2024.,
        };
        let source_bounds = MinmaxBounds { min: 0., max: 7. };
        let weight = judweight_depth();

        let result: Vec<_> = slice_source
            .iter()
            .zip(slice_year)
            .map(|(&s, y)| {
                (
                    1. - relative_to_bounds(source_bounds, s as f64),
                    relative_to_bounds(age_bounds, y as f64),
                )
            })
            .map(|(s, y)| weight[0] * s + (weight[1] * y).max(0.).min(1.))
            .collect();

        let mut out = output.flat_vector();
        out.copy(&result);
        // _output.flat_vector()
        // let some = out.as_mut_slice_with_len(3);
        assert_eq!(input.len(), result.len());

        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![
                LogicalTypeHandle::from(LogicalTypeId::UTinyint),
                LogicalTypeHandle::from(LogicalTypeId::UInteger),
            ],
            LogicalTypeHandle::from(LogicalTypeId::Double),
        )]
    }
}

struct ExtractTrajectories;

impl VScalar for ExtractTrajectories {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input_len = input.len();
        let from_point = input.struct_vector(0);
        let to_point = input.struct_vector(1);
        let from_lon_flat = from_point.child(0, input_len);
        let from_lon_s: &[f32] = from_lon_flat.as_slice_with_len(input_len);
        let from_lat_flat = from_point.child(1, input_len);
        let from_lat_s: &[f32] = from_lat_flat.as_slice_with_len(input_len);
        let from_time_flat = from_point.child(2, input_len);
        let from_time_s: &[f64] = from_time_flat.as_slice_with_len(input_len);
        let to_lon_flat = to_point.child(0, input_len);
        let to_lon_s: &[f32] = to_lon_flat.as_slice_with_len(input_len);
        let to_lat_flat = to_point.child(1, input_len);
        let to_lat_s: &[f32] = to_lat_flat.as_slice_with_len(input_len);
        let to_time_flat = to_point.child(2, input_len);
        let to_time_s: &[f64] = to_time_flat.as_slice_with_len(input_len);

        let from_point = izip!(from_lon_s, from_lat_s, from_time_s)
            .map(|(&lon, &lat, &t)| PointM::<4326>::from((lon as f64, lat as f64, t)));
        let to_point = izip!(to_lon_s, to_lat_s, to_time_s)
            .map(|(&lon, &lat, &t)| PointM::<4326>::from((lon as f64, lat as f64, t)));

        let result: Vec<_> = from_point
            .zip(to_point)
            .map(|(from, to)| dist(from, to, 1000_f64) && time_dist(from, to, 60_f64))
            .collect();
        let mut out = output.flat_vector();
        out.copy(&result);

        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        let point = [
            ("lon", LogicalTypeHandle::from(LogicalTypeId::Float)),
            ("lat", LogicalTypeHandle::from(LogicalTypeId::Float)),
            ("time", LogicalTypeHandle::from(LogicalTypeId::Double)),
        ];
        let params = vec![
            LogicalTypeHandle::struct_type(&point),
            LogicalTypeHandle::struct_type(&point),
        ];
        vec![ScalarFunctionSignature::exact(
            params,
            LogicalTypeHandle::from(LogicalTypeId::Boolean),
        )]
    }
}

fn dist(first: PointM, second: PointM, thres: f64) -> bool {
    use geo::algorithm::line_measures::metric_spaces::Geodesic;
    Geodesic.distance(first, second) < thres
}

const fn time_dist(first: PointM, second: PointM, thres: f64) -> bool {
    second.coord.m - first.coord.m < thres
}
