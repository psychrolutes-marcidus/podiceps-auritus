use std::{error::Error, ffi::c_int, sync::atomic::AtomicBool};

use duckdb::{
    core::{Inserter, LogicalTypeHandle, LogicalTypeId},
    duckdb_entrypoint_c_api,
    vtab::VTab,
    Connection,
};

// pub mod etl;

const EXTENSION_NAME: &str = env!("CARGO_PKG_NAME");

#[repr(C)]
struct HelloBindData {
    name: String,
}

#[repr(C)]
struct HelloInitData {
    done: AtomicBool,
}

struct HelloVTab;

impl VTab for HelloVTab {
    type InitData = HelloInitData;

    type BindData = HelloBindData;

    fn bind(
        bind: &duckdb::vtab::BindInfo,
    ) -> duckdb::Result<Self::BindData, Box<dyn std::error::Error>> {
        bind.add_result_column("type", LogicalTypeHandle::from(LogicalTypeId::Integer));
        let name = bind.get_parameter(0).to_string();
        Ok(HelloBindData { name })
    }

    fn init(
        _init: &duckdb::vtab::InitInfo,
    ) -> duckdb::Result<Self::InitData, Box<dyn std::error::Error>> {
        Ok(HelloInitData {
            done: AtomicBool::new(false),
        })
    }

    fn func(
        func: &duckdb::vtab::TableFunctionInfo<Self>,
        output: &mut duckdb::core::DataChunkHandle,
    ) -> duckdb::Result<(), Box<dyn std::error::Error>> {
        let init_data = func.get_init_data();
        let bind_data = func.get_bind_data();
        if init_data
            .done
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            output.set_len(0);
        } else {
            let mut vector = output.flat_vector(0);
            let data = vec![42_i32, 60_i32];
            vector.copy(&data);
            output.set_len(2);
        }
        Ok(())
    }
    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        Some(vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)])
    }
}

#[no_mangle]
pub unsafe extern "C" fn ferruginous_init_c_api() {
    println!("Hello");
}
