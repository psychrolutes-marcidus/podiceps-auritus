use std::{any::Any, error::Error};

use duckdb::{
    core::{LogicalTypeHandle, LogicalTypeId},
    vscalar::{ScalarFunctionSignature, VScalar},
    Connection,
};
use libduckdb_sys::duckdb_aggregate_function;
use libduckdb_sys::duckdb_register_aggregate_function;

pub fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn Error>> {
    unsafe { duckdb_register_aggregate_function(con, aggr_function) }

    Ok(())
}

#[repr(C)]
struct StateExtractDraught {
    len: usize,
}

struct ExtractDraught;

impl VScalar for ExtractDraught {
    type State = StateExtractDraught;

    unsafe fn invoke(
        state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // if state.len == 0 && input.len() != 0 {
        //     state.len = input.len();
        // }
        // let flat_vec = input.flat_vector(0);
        // let len = vec![input.len()];
        // let num: &[i32] = flat_vec.as_slice();
        // let result: Vec<_> = num.iter().map(|x| x + 1).collect();
        // output.flat_vector().copy(&len);
        Ok(())
    }

    fn signatures() -> Vec<duckdb::vscalar::ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![LogicalTypeHandle::from(LogicalTypeId::Integer)],
            LogicalTypeHandle::from(LogicalTypeId::Integer),
        )]
    }
}

struct StateDDMReliability {
    len: usize,
}

struct DDMReliability;

impl VScalar for DDMReliability {
    type State = StateDDMReliability;

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        _output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = input.flat_vector(0);
        let year = input.flat_vector(1);

        let slice_source: &[u8] = source.as_slice();
        let slice_year: &[u32] = year.as_slice();

        todo!()
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![
                LogicalTypeHandle::from(LogicalTypeId::UTinyint),
                LogicalTypeHandle::from(LogicalTypeId::UInteger),
            ],
            LogicalTypeHandle::from(LogicalTypeId::Float),
        )]
    }
}
