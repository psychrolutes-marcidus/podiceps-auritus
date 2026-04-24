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
        let draught_max_flat = input.flat_vector(0);
        let draught_max_s: &[f32] = draught_max_flat.as_slice_with_len(input_len);
        let draught_max_s = draught_max_s
            .iter()
            .enumerate()
            .map(|(i, &v)| match draught_max_flat.row_is_null(i as u64) {
                true => None,
                false => Some(v),
            });
        let score_max_flat = input.flat_vector(1);
        let score_max_s: &[f32] = score_max_flat.as_slice_with_len(input_len);
        let score_max_s = score_max_s.iter().enumerate().map(|(i, &v)| {
            match score_max_flat.row_is_null(i as u64) {
                true => None,
                false => Some(v),
            }
        });
        let std_max_flat = input.flat_vector(2);
        let std_max_s: &[f32] = std_max_flat.as_slice_with_len(input_len);
        let std_max_s =
            std_max_s
                .iter()
                .enumerate()
                .map(|(i, &v)| match std_max_flat.row_is_null(i as u64) {
                    true => None,
                    false => {
                        if v == 0. {
                            return None;
                        }
                        Some(v)
                    }
                });
        let draught_other_flat = input.flat_vector(3);
        let draught_other_s: &[f32] = draught_other_flat.as_slice_with_len(input_len);
        let draught_other_s = draught_other_s.iter().enumerate().map(|(i, &v)| {
            match draught_other_flat.row_is_null(i as u64) {
                true => None,
                false => Some(v),
            }
        });
        let score_other_flat = input.flat_vector(4);
        let score_other_s: &[f32] = score_other_flat.as_slice_with_len(input_len);
        let score_other_s = score_other_s
            .iter()
            .enumerate()
            .map(|(i, &v)| match score_other_flat.row_is_null(i as u64) {
                true => None,
                false => Some(v),
            });
        let std_other_flat = input.flat_vector(5);
        let std_other_s: &[f32] = std_other_flat.as_slice_with_len(input_len);
        let std_other_s = std_other_s.iter().enumerate().map(|(i, &v)| {
            match std_other_flat.row_is_null(i as u64) {
                true => None,
                false => {
                    if v == 0. {
                        return None;
                    }
                    Some(v)
                }
            }
        });

        let result: Vec<_> = draught_max_s
            .zip(score_max_s)
            .zip(std_max_s)
            .zip(draught_other_s.zip(score_other_s).zip(std_other_s))
            .map(|(((d_m, s_m), dev_m), ((d_o, s_o), dev_o))| {
                d_m.zip(s_m)
                    .zip(dev_m)
                    .zip(d_o.zip(s_o).zip(dev_o))
                    .map(|(m, o)| combine_cell([m.0.0, o.0.0], [m.0.1, o.0.1], [m.1, o.1]))
                    .unwrap_or_default()
            })
            .collect();

        let mut output_vec = output.flat_vector();
        output_vec.copy(&result);

        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        let params = vec![
            LogicalTypeHandle::from(LogicalTypeId::Float), // Draught max
            LogicalTypeHandle::from(LogicalTypeId::Float), // Score max
            LogicalTypeHandle::from(LogicalTypeId::Float), // Dev max
            LogicalTypeHandle::from(LogicalTypeId::Float), // Draught other
            LogicalTypeHandle::from(LogicalTypeId::Float), // Score other
            LogicalTypeHandle::from(LogicalTypeId::Float), // Dev other
        ];

        vec![ScalarFunctionSignature::exact(
            params,
            LogicalTypeHandle::from(LogicalTypeId::Float),
        )]
    }
}

fn combine_cell(draught: [f32; 2], score: [f32; 2], deviation: [f32; 2]) -> f32 {
    gravity_model(
        score[0],
        draught[0],
        deviation[0],
        score[1],
        draught[1],
        deviation[1],
    )
}
