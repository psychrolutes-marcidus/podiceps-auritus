use std::{error::Error, sync::atomic::AtomicBool};

use duckdb::{
    Connection,
    core::{LogicalTypeHandle, LogicalTypeId},
    duckdb_entrypoint_c_api,
    vtab::VTab,
};
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
        bind.add_result_column("column0", LogicalTypeHandle::from(LogicalTypeId::Varchar));
        let name = bind.get_parameter(0).to_string();
        Ok(HelloBindData { name })
    }

    fn init(
        init: &duckdb::vtab::InitInfo,
    ) -> duckdb::Result<Self::InitData, Box<dyn std::error::Error>> {
        todo!()
    }

    fn func(
        func: &duckdb::vtab::TableFunctionInfo<Self>,
        output: &mut duckdb::core::DataChunkHandle,
    ) -> duckdb::Result<(), Box<dyn std::error::Error>> {
        todo!()
    }
}

#[duckdb_entrypoint_c_api()]
pub unsafe fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn Error>> {
    con.register_table_function::<HelloVTab>(EXTENSION_NAME)
        .expect("Failed to register hello table function");
    Ok(())
}
