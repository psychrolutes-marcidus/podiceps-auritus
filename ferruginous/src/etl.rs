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

#[no_mangle]
pub extern "C" fn aggr_function(_data: u8) {}

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
