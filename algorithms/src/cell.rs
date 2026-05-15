use approx::AbsDiffEq;
use cached::proc_macro::once;
use std::{
    f64::consts::PI,
    ops::{Div, Sub},
};

use geo_types::polygon;
use nalgebra::{ArrayStorage, Const, Matrix, matrix, vector};

#[derive(Debug, Clone, Copy)]
pub struct MinmaxBounds<T>
where
    T: PartialOrd,
{
    pub min: T,
    pub max: T,
}
const TOL: f32 = 1e-6;
const RADIUS: f64 = 6378137.0;
const CIRCUMFERENCE: f64 = 2. * PI * RADIUS;

#[once]
pub fn judweight_vessel() -> [f32; 6] {
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
    mut eig_val: Matrix<f32, Const<P>, Const<1>, ArrayStorage<f32, P, 1>>,
    mat: Matrix<f32, Const<P>, Const<P>, ArrayStorage<f32, P, P>>,
) -> [f32; P] {
    let mut output: [f32; P] = [0.; P];
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
    norm_vec
        .iter()
        .cloned()
        .enumerate()
        .for_each(|(i, v)| output[i] = v);
    output
}

#[once]
pub fn judweight_depth() -> [f32; 2] {
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

pub fn gravity_model(
    rel_m: f32,
    draught_m: f32,
    dev_m: f32,
    rel_o: f32,
    draught_o: f32,
    dev_o: f32,
) -> f32 {
    let diff = draught_m - draught_o;
    let rel = rel_m * rel_o;
    if rel > 1.1 {
        dbg!(&rel_m);
        dbg!(&rel_o);
    }
    assert!(rel <= 1.1);
    let dr_m = draught_dev(draught_m, dev_m);
    let dr_o = draught_dev(draught_o, dev_o);
    let _dev = dr_m * dr_o;
    let n = diff;
    // if diff < 1. {
    //     (rel) / (dev)
    // } else if diff < 2. {
    //     (rel) / (dev * (diff).powi(2))
    // } else {
    //     (rel) / (4. * dev)
    // }

    rel / (n + 1.)
}

fn draught_dev(draught: f32, med: f32) -> f32 {
    (draught - med).abs() / med
}

// Taken from the DuckDB spatial extension implementation.
pub fn st_tileenvelope(z: u32, x: i32, y: i32) -> geo::Polygon<f64> {
    let zoom_extent = (1_u32 << z) as f64;

    let single_tile_width = CIRCUMFERENCE / zoom_extent;
    let single_tile_height = CIRCUMFERENCE / zoom_extent;
    let tile_left = get_tile_left(x as u32, single_tile_width);
    let tile_right = tile_left + single_tile_width;
    let tile_top = get_tile_top(y as u32, single_tile_height);
    let tile_bottom = tile_top - single_tile_height;

    init_from_bbox(tile_left, tile_bottom, tile_right, tile_top)
}

fn get_tile_left(tile_x: u32, single_tile_width: f64) -> f64 {
    -0.5 * CIRCUMFERENCE + (tile_x as f64 * single_tile_width)
}

fn get_tile_top(tile_y: u32, single_tile_height: f64) -> f64 {
    0.5 * CIRCUMFERENCE - (tile_y as f64 * single_tile_height)
}
fn init_from_bbox(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> geo::Polygon<f64> {
    polygon!((x: min_x, y:min_y), (x: min_x, y:max_y),(x: max_x, y:max_y),(x: max_x, y:min_y), (x: min_x, y: min_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gives_result() {
        let vec: [f32; 2] = judweight_depth().as_array().unwrap().to_owned();
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

    // It is a good idea to make tests
    #[test]
    fn tile_envelope_test() {
        let world = st_tileenvelope(0, 0, 0);
        let aalborg = st_tileenvelope(10, 540, 313);

        let world_text = format!("{:?}", world);
        let aalborg_text = format!("{:?}", aalborg);

        assert_eq!(
            world_text,
            "POLYGON((-20037508.342789244 -20037508.342789244,-20037508.342789244 20037508.342789244,20037508.342789244 20037508.342789244,20037508.342789244 -20037508.342789244,-20037508.342789244 -20037508.342789244))"
        );

        assert_eq!(
            aalborg_text,
            "POLYGON((1095801.2374962866 7748880.179438028,1095801.2374962866 7788015.937920038,1134936.995978297 7788015.937920038,1134936.995978297 7748880.179438028,1095801.2374962866 7748880.179438028))"
        );
    }
}
