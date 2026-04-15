use approx::AbsDiffEq;
use cached::proc_macro::once;
use std::ops::{Div, Sub};

use nalgebra::{ArrayStorage, Const, Matrix, matrix, vector};

#[derive(Debug, Clone, Copy)]
pub struct MinmaxBounds<T>
where
    T: PartialOrd,
{
    pub min: T,
    pub max: T,
}
const TOL: f64 = 1e-6;

#[once]
pub fn judweight_vessel() -> Vec<f64> {
    let eig_val = vector![0., 1., 2., 3., 4., 5.];
    let a12 = 2.;
    let a13 = 5.;
    let a14 = 1.;
    let a15 = 3.;
    let a16 = 2.;
    let a23 = 3.;
    let a24 = 1. / 2.;
    let a25 = 2.;
    let a26 = 2.;
    let a34 = 1. / 3.;
    let a35 = 1.;
    let a36 = 1.;
    let a45 = 1. / 3.;
    let a46 = 1. / 3.;
    let a56 = 2.;

    let mat = matrix![1., a12, a13, a14, a15, a16; 1./a12, 1., a23, a24, a25, a26; 1./a13, 1./a23, 1., a34, a35, a36; 1./a14, 1./a24, 1./a34, 1., a45, a46; 1./a15, 1./a25, 1./a35, 1./a45, 1., a56; 1./a16, 1./a26, 1./a36, 1./a46, 1./a56, 1. ];

    // Using the power method to approximate the largest eigenvalues
    power(eig_val, mat)
}

fn power<const P: usize>(
    mut eig_val: Matrix<f64, Const<P>, Const<1>, ArrayStorage<f64, P, 1>>,
    mat: Matrix<f64, Const<P>, Const<P>, ArrayStorage<f64, P, P>>,
) -> Vec<f64> {
    let mut prev_eig_val = eig_val.clone();
    loop {
        let new_eig_val = mat * eig_val;
        let new_eig_val_norm = new_eig_val.norm();
        eig_val = new_eig_val / new_eig_val_norm;
        // Check if the new weights has converged
        if eig_val.abs_diff_eq(&prev_eig_val, TOL) {
            break;
        }
        prev_eig_val = eig_val.clone();
    }
    let sum = eig_val.sum();
    let norm_vec = eig_val.scale(1. / sum);
    norm_vec.iter().cloned().collect::<Vec<f64>>()
}

#[once]
pub fn judweight_depth() -> Vec<f64> {
    let eig_val = vector![0., 1.];
    let a12 = 3.;
    let mat = matrix![1., a12; 1. / a12, 1.];

    // Using the power method to approximate the largest eigenvalues
    power(eig_val, mat)
}

pub fn relative_to_bounds<T>(bounds: MinmaxBounds<T>, num: T) -> T
where
    T: PartialOrd + Sub<Output = T> + Div<Output = T> + Copy,
{
    (num - bounds.min) / (bounds.max - bounds.min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gives_result() {
        let vec: [f64; 2] = judweight_depth().as_array().unwrap().to_owned();
        let source_bounds = MinmaxBounds { min: 0., max: 7. };
        let age_bounds = MinmaxBounds {
            min: 2000.,
            max: 2024.,
        };

        let source = 0.;
        let age = 2024.;
        let result_source = 1. - relative_to_bounds(source_bounds, source);
        let result_age = relative_to_bounds(age_bounds, age);

        let result = result_source * vec[0] + result_age * vec[1];

        assert_eq!(result, 1.);
        let source = 7.;
        let age = 2000.;
        let result_source = 1. - relative_to_bounds(source_bounds, source);
        let result_age = relative_to_bounds(age_bounds, age);

        let result = result_source * vec[0] + result_age * vec[1];
        assert_eq!(result, 0.);
    }
}
