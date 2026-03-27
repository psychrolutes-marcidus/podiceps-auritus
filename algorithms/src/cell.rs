use std::ops::{Div, Sub};

use nalgebra::{matrix, vector};

#[derive(Debug, Clone, Copy)]
pub struct MinmaxBounds<T>
where
    T: PartialOrd,
{
    pub min: T,
    pub max: T,
}

pub fn judweight_vessel() -> Vec<f64> {
    let mut vec = vector![0., 1., 2., 3., 4., 5.];
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
    for _ in 0..100 {
        let some = mat * vec;
        let some_norm = some.norm();
        vec = some / some_norm;
    }
    let sum = vec.sum();
    let norm_vec = vec.scale(1. / sum);
    norm_vec.iter().cloned().collect::<Vec<f64>>()
}

pub fn judweight_depth() -> Vec<f64> {
    let mut vec = vector![0., 1.];
    let a12 = 3.;
    let mat = matrix![1., a12; 1. / a12, 1.];
    for _ in 0..100 {
        let some = mat * vec;
        let some_norm = some.norm();
        vec = some / some_norm;
    }
    let sum = vec.sum();
    let norm_vec = vec.scale(1. / sum);
    norm_vec.iter().cloned().collect::<Vec<f64>>()
}

pub fn between_min_max<T>(bounds: MinmaxBounds<T>, num: T) -> T
where
    T: PartialOrd + Sub<Output = T> + Div<Output = T> + Copy,
{
    (num - bounds.min) / (bounds.max - bounds.min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let vec: [f64; 2] = judweight_depth().as_array().unwrap().to_owned();
        let source_bounds = MinmaxBounds { min: 0., max: 7. };
        let age_bounds = MinmaxBounds {
            min: 2000.,
            max: 2024.,
        };

        let source = 3.;
        let age = 2023.;
        let result_source = 1. - between_min_max(source_bounds, source);
        let result_age = between_min_max(age_bounds, age);

        let result = result_source * vec[0] + result_age * vec[1];
        dbg!(&result);

        // let vec = judweight_vessel();
        dbg!(&vec);

        assert!(false);
    }
}
