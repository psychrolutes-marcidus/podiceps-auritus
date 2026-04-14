use duckdb::{
    Connection,
    core::{LogicalTypeHandle, LogicalTypeId},
    vscalar::{ScalarFunctionSignature, VScalar},
};

pub fn extension_entrypoint(con: &Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<EvalCell>("eval_cell")?;
    Ok(())
}

struct EvalCell;

impl VScalar for EvalCell {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        _output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input1 = input.flat_vector(0);
        let input1_s: &[f32] = input1.as_slice_with_len(input.len());
        let input2 = input.flat_vector(1);
        let input2_s: &[f32] = input2.as_slice_with_len(input.len());
        let input3 = input.flat_vector(2);
        let input3_s: &[f32] = input3.as_slice_with_len(input.len());
        let input4 = input.flat_vector(3);
        let input4_s: &[f32] = input4.as_slice_with_len(input.len());
        let input5 = input.flat_vector(4);
        let input5_s: &[f32] = input5.as_slice_with_len(input.len());
        let input6 = input.flat_vector(5);
        let input6_s: &[f32] = input6.as_slice_with_len(input.len());

        todo!()
    }

    fn signatures() -> Vec<duckdb::vscalar::ScalarFunctionSignature> {
        let params = vec![
            LogicalTypeHandle::from(LogicalTypeId::Float),
            LogicalTypeHandle::from(LogicalTypeId::Float),
            LogicalTypeHandle::from(LogicalTypeId::Float),
            LogicalTypeHandle::from(LogicalTypeId::Float),
            LogicalTypeHandle::from(LogicalTypeId::Float),
            LogicalTypeHandle::from(LogicalTypeId::Float),
        ];
        let output = LogicalTypeHandle::from(LogicalTypeId::Float);
        vec![ScalarFunctionSignature::exact(params, output)]
    }
}
