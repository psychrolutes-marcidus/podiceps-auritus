use std::error::Error;

use algorithms::cell::gravity_model;
use duckdb::{
    Connection,
    core::{LogicalTypeHandle, LogicalTypeId},
    vscalar::{ScalarFunctionSignature, VScalar},
};
use itertools::Itertools;

pub fn extension_entrypoint(con: &Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<EvalCell>("eval_cell")?;
    con.register_scalar_function::<CombineCell>("combine_cell")?;
    Ok(())
}

struct EvalCell;

impl VScalar for EvalCell {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input_len = input.len();
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
        let input1_nulls = (0..input.len()).map(|i| input1.row_is_null(i as u64));
        let input2_nulls = (0..input.len()).map(|i| input2.row_is_null(i as u64));
        let input3_nulls = (0..input.len()).map(|i| input3.row_is_null(i as u64));
        let input4_nulls = (0..input.len()).map(|i| input4.row_is_null(i as u64));
        let input5_nulls = (0..input.len()).map(|i| input5.row_is_null(i as u64));
        let input6_nulls = (0..input.len()).map(|i| input6.row_is_null(i as u64));
        let input1_option = input1_s.iter().zip(input1_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });
        let input2_option = input2_s.iter().zip(input2_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });
        let input3_option = input3_s.iter().zip(input3_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });
        let input4_option = input4_s.iter().zip(input4_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });
        let input5_option = input5_s.iter().zip(input5_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });
        let input6_option = input6_s.iter().zip(input6_nulls).map(|(&d, n)| match n {
            false => Some(d),
            true => None,
        });
        let weights = algorithms::cell::judweight_vessel();

        let mut output_vec = output.flat_vector();
        let mut nulls = Vec::new();

        let score: Vec<_> = input1_option
            .zip(input2_option)
            .zip(input3_option)
            .zip(input4_option)
            .zip(input5_option)
            .zip(input6_option)
            .map(|(((((input1, input2), input3), input4), input5), input6)| {
                input1
                    .zip(input2)
                    .zip(input3)
                    .zip(input4)
                    .zip(input5)
                    .zip(input6)
                    .map(|(((((input1, input2), input3), input4), input5), input6)| {
                        [input1, input2, input3, input4, input5, input6]
                    })
            })
            .map(|row| {
                row.map(|x| {
                    x.iter()
                        .zip(weights.iter())
                        .map(|(&a, &b)| a * b)
                        .sum::<f32>()
                })
            })
            .enumerate()
            .inspect(|(i, v)| match v {
                None => {
                    nulls.push(*i);
                }
                _ => {}
            })
            .map(|(_, v)| v.unwrap_or_default())
            .collect();

        output_vec.copy(&score);
        nulls.iter().for_each(|i| output_vec.set_null(*i));
        debug_assert_eq!(input_len, score.len());
        Ok(())
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

struct CombineCell;

impl VScalar for CombineCell {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut duckdb::core::DataChunkHandle,
        output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input_len = input.len();
        let draught_val = input.list_vector(0);
        let draught_flat = draught_val.child(draught_val.len());
        let draught_s: &[f32] = draught_flat.as_slice_with_len(draught_val.len());
        let draught_s = (0..input_len)
            .map(|x| draught_val.get_entry(x))
            .map(|(off, len)| &draught_s[off..off + len]);
        let score_val = input.list_vector(1);
        let score_flat = score_val.child(score_val.len());
        let score_s: &[f32] = score_flat.as_slice_with_len(score_val.len());
        let score_s = (0..input_len)
            .map(|x| score_val.get_entry(x))
            .map(|(off, len)| &score_s[off..off + len]);
        let std_val = input.list_vector(2);
        let std_flat = std_val.child(std_val.len());
        let std_s: &[f32] = std_flat.as_slice_with_len(std_val.len());
        let std_s = (0..input_len)
            .map(|x| std_val.get_entry(x))
            .map(|(off, len)| &std_s[off..off + len]);

        let (draught, score): (Vec<_>, Vec<_>) = draught_s
            .zip(score_s)
            .zip(std_s)
            .map(|((d, s), std)| {
                let ((d, s), std): ((Vec<f32>, Vec<f32>), Vec<f32>) = d
                    .iter()
                    .zip(s.iter())
                    .zip(std.iter())
                    .sorted_by(|a, b| a.0.0.total_cmp(b.0.0))
                    .rev()
                    .unzip();
                combine_cell(&d, &s, &std, 0.53)
            })
            .unzip();

        let out_struct = output.struct_vector();
        let mut draught_out = out_struct.child(0, input_len);
        let mut reliability_out = out_struct.child(1, input_len);
        draught_out.copy(&draught);
        reliability_out.copy(&score);

        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        let params = vec![
            LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Float)), // Draught values
            LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Float)), // Score values
            LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Float)), // Standard deviation on draught values
        ];

        let out_struct = [
            ("draught", LogicalTypeHandle::from(LogicalTypeId::Float)),
            ("reliability", LogicalTypeHandle::from(LogicalTypeId::Float)),
        ];

        let return_type = LogicalTypeHandle::struct_type(&out_struct);
        vec![ScalarFunctionSignature::exact(params, return_type)]
    }
}

fn combine_cell(draught: &[f32], score: &[f32], deviation: &[f32], thres: f32) -> (f32, f32) {
    if draught.len() == 0 {
        return (0., 0.);
    }
    let output = draught
        .iter()
        .zip(score.iter())
        .zip(deviation.iter())
        .map(|((&d, &s), &std)| gravity_model(score[0], draught[0], deviation[0], s, d, std))
        .max_by(|a, b| a.total_cmp(b))
        .map(|x| (draught[0], x))
        .unwrap_or((0., 0.));

    if output.1 < thres {
        return combine_cell(&draught[1..], &score[1..], &deviation[1..], thres);
    }
    return output;
}
