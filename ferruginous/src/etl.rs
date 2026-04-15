use algorithms::cell::MinmaxBounds;
use algorithms::cell::judweight_depth;
use algorithms::cell::relative_to_bounds;
use std::error::Error;

use duckdb::{
    Connection,
    core::{LogicalTypeHandle, LogicalTypeId},
    vscalar::{ScalarFunctionSignature, VScalar},
};

pub fn extension_entrypoint(con: &Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<DDMReliability>("ddm_reliability")?;

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
