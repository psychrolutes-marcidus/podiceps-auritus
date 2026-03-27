use chrono::DateTime;
use chrono::TimeDelta;
use chrono::Timelike;
use chrono::Utc;
use geo::Distance;
use linesonmaps::algo::segmenter::segment_timestamp;
use linesonmaps::types::coordm::CoordM;
use linesonmaps::types::pointm::PointM;
use std::error::Error;

use duckdb::{
    Connection,
    core::{LogicalTypeHandle, LogicalTypeId},
    vscalar::{ScalarFunctionSignature, VScalar},
};

pub fn extension_entrypoint(con: &Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<ExtractTrajectories>("trajectory_split")?;

    Ok(())
}

#[repr(C)]
#[derive(Default)]
struct StateExtractTrajectories {
    len: usize,
}

struct ExtractTrajectories;

impl VScalar for ExtractTrajectories {
    type State = StateExtractTrajectories;

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let lon_list = input.list_vector(0);
        let lat_list = input.list_vector(1);
        let time_list = input.list_vector(2);
        let lon_flat = lon_list.child(lon_list.len());
        let lat_flat = lat_list.child(lat_list.len());
        let time_flat = time_list.child(time_list.len());
        let lon_sl: &[f32] = lon_flat.as_slice();
        let lat_sl: &[f32] = lat_flat.as_slice();
        let time_sl: &[f64] = time_flat.as_slice();

        // let lon = input_val_struct.list_vector_child(0);
        // let lat = input_val_struct.list_vector_child(1);
        // let time = input_val_struct.list_vector_child(2);
        // // dbg!(&lon.len());
        // // dbg!(&lat.len());
        // // dbg!(&time.len());
        // let lon_flat = lon.child(lon.len());
        // let lon_slice: &[f32] = lon_flat.as_slice();
        // let lat_flat = lat.child(lat.len());
        // let lat_slice: &[f32] = lat_flat.as_slice();
        // // let time_flat = time.child(time.len());
        // // let time_slice: &[DateTime<Utc>] = time_flat.as_slice();
        // // let func = |f, l| dist(f, l, 1000_f64) && time_dist(f, l, 60_f64);
        // dbg!(&lon_sl);
        let mut coords: Vec<CoordM<4326>> = lon_sl
            .iter()
            .zip(lat_sl.iter())
            .zip(time_sl.iter())
            .map(|((&x, &y), &t)| CoordM {
                x: x.into(),
                y: y.into(),
                m: t,
            })
            .collect();
        coords.sort_by(|a, b| a.m.total_cmp(&b.m));
        dbg!(&coords.last().unwrap().m);

        // let ls = linesonmaps::types::linestringm::LineStringM::<4326>::new(coords).unwrap();
        // // let segments = segment_timestamp(ls, func);
        // // let (time_begin, delta): (Vec<i64>, Vec<i64>) = segments
        // //     .into_iter()
        // //     .map(|x| (x.0.timestamp(), x.1.num_seconds()))
        // //     .unzip();

        // println!("Hello");

        // // let output_vec = output.struct_vector();
        // let time_v = output_vec.list_vector_child(0);

        let str_out = output.struct_vector();
        let _flat_out = str_out.child(0, 0);

        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        let return_field = [
            (
                "time_begin",
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Timestamp)),
            ),
            (
                "duration",
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Interval)),
            ),
        ];
        vec![ScalarFunctionSignature::exact(
            vec![
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Float)),
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Float)),
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Double)),
            ],
            LogicalTypeHandle::struct_type(&return_field),
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
