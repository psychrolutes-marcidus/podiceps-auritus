use std::collections::HashSet;
use std::f64;

use geo::{Coord, Distance, GeoNum, Geodesic, Point};
use linesonmaps::algo::stop_cluster::DbScanConf;
use linesonmaps::types::{linestringm::LineStringM, pointm::PointM};
use tilerizer::{Point as GPoint, PointWTime, draw_2d_vessel, draw_linestring, point_to_grid};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, Copy, TypedBuilder)]
pub struct ErrorMeasurementConf {
    method: ErrorMeasurementMethod,
    rendering_model: RenderingModel,
    zoom: u8,
    #[builder(default, setter(strip_option))]
    sampling: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorMeasurementMethod {
    /// Measures geodesic distance from cell centroid to nearest ground truth point.
    Geodesic,
    /// measures distance in terms of horizontal cell distance + vertical cell distance
    CellTaxicab,
}
#[derive(Debug, Clone, Copy)]
pub enum RenderingModel {
    /// Model ship movement as a linestring
    Linestring,
    /// Model ship movement as a moving polygon (i.e. linestring with width)
    TwoDimensional { a: u16, b: u16, c: u16, d: u16 },
}

impl ErrorMeasurementConf {
    //TODO maybe there should be a function here for aggregating errors across multiple trajectories, but i do not know if it needs any more parameters
    pub fn measure_error(self, ls: &LineStringM<4326>) -> Vec<(GPoint, f64)> {
        self.calculate_error(
            ls,
            &self
                .generate_cells(ls)
                .difference(&self.ground_truth_cells(ls))
                .cloned()
                .collect(),
        )
    }
    fn calculate_error(
        &self,
        gt_ls: &LineStringM<4326>,
        cells: &HashSet<GPoint>,
    ) -> Vec<(GPoint, f64)> {
        let ground_truth_cells = self.ground_truth_cells(&gt_ls);
        debug_assert!(
            ground_truth_cells.intersection(cells).count() == 0,
            "cells should be disjoint with ground-truth cells"
        );
        cells
            .iter()
            .map(|c| (*c, self.cell_to_nearest_point(gt_ls, c)))
            .collect()
    }
    fn cell_to_nearest_point(&self, gt: &LineStringM<4326>, gp: &GPoint) -> f64 {
        match self.method {
            ErrorMeasurementMethod::CellTaxicab => gt
                .points()
                .map(|p| point_to_grid(p.coord.into(), self.zoom.into()))
                .map(|c| (c.x - gp.x).abs() + (c.y - gp.y).abs())
                .min_by(|x, y| x.total_cmp(y))
                .unwrap_or(0) as f64,
            ErrorMeasurementMethod::Geodesic => gt
                .points()
                .map(|p| ground_truth_to_cell_geodesic(p, &gp, self.zoom))
                .min_by(|x, y| x.total_cmp(y))
                .unwrap_or(0.0),
        }
    }
    fn generate_cells(&self, gt_ls: &LineStringM<4326>) -> HashSet<GPoint> {
        let points = match self.rendering_model {
            RenderingModel::Linestring => draw_linestring(
                &[&gt_ls],
                self.zoom.into(),
                self.sampling.unwrap_or(self.zoom).into(),
                None,
            ),
            RenderingModel::TwoDimensional { a, b, c, d } => draw_2d_vessel(
                &[&gt_ls],
                a as i16,
                b as i16,
                c as i16,
                d as i16,
                self.zoom.into(),
                self.sampling.unwrap_or(self.zoom).into(),
                None,
            ),
        };
        points
            .into_iter()
            .map(|pw| pw.point)
            .collect::<HashSet<GPoint>>()
            .difference(&self.ground_truth_cells(gt_ls))
            .cloned()
            .collect()
    }
    fn ground_truth_cells(&self, gt_ls: &LineStringM<4326>) -> HashSet<GPoint> {
        gt_ls
            .points()
            .map(|p| point_to_grid(p.coord.into(), self.zoom.into()))
            .collect()
    }
}

// implementation based on https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
fn grid_centroid_to_lng_lat(gp: GPoint, zoom: u8) -> Point<f64> {
    //TODO: this might map to the northwesternmost point in a grid cell, correct behavior should be centroid
    // seems to be close enough
    let lon = ((0.5 + gp.x as f64) / (2_f64.powi(zoom as i32))) * 360_f64 - 180_f64;
    let lat = (f64::consts::PI
        - ((0.5 + gp.y as f64) / 2_f64.powi(zoom as i32) * 2_f64 * f64::consts::PI))
        .sinh()
        .atan()
        * (180_f64 / f64::consts::PI);
    Point(Coord { x: lon, y: lat })
}

/// Measures sum of error for cells, at the given zoom level, that do not contain any ground-truth point. Points in the linestring are considered ground-truth point
#[deprecated = "use `ErrorMeasurementConf` instead"]
fn line_error_from_ground_truth_geodesic(
    ls: &LineStringM<4326>,
    zoom: i32,
    sampling_zoom: i32,
) -> Vec<(GPoint, f64)> {
    let ground_truth = ls.points();
    let ground_truth_cells = ground_truth.map(|p| point_to_grid(p.coord.into(), zoom));
    let cells = draw_linestring(&[&ls], zoom, sampling_zoom, None)
        .into_iter()
        .map(|pw| pw.point)
        .collect::<HashSet<GPoint>>();

    let ground_truth_hashset = HashSet::from_iter(ground_truth_cells);
    let cells_diff = cells
        .difference(&ground_truth_hashset)
        .cloned()
        .collect::<Vec<_>>(); // cells that are not ground truth cells

    // for each non-ground truth cell, find geodesic distance to nearest ground-truth point
    let cell_errors = cells_diff
        .into_iter()
        .map(|c| {
            (
                c,
                ls.points()
                    .map(move |gtp| ground_truth_to_cell_geodesic(gtp, &c, zoom as u8))
                    .min_by(|x, y| x.total_cmp(y))
                    .unwrap_or(0.0),
            )
        })
        .collect::<Vec<_>>();
    cell_errors
}

fn ground_truth_to_cell_geodesic<P: Into<Point<f64>>>(p: P, gp: &GPoint, zoom: u8) -> f64 {
    Geodesic.distance(grid_centroid_to_lng_lat(*gp, zoom), p.into())
}

/// Measures sum of error for cells, at the given zoom level, that do not contain any ground-truth point. Points in the linestring are considered ground-truth point
#[deprecated = "use `ErrorMeasurementConf` instead"]
fn line_error_from_ground_truth(
    ls: &LineStringM<4326>,
    zoom: i32,
    sampling_zoom: i32,
) -> Vec<(GPoint, i32)> {
    let ground_truth = ls.points();
    let ground_truth_cells = ground_truth
        .map(|p| point_to_grid(p.coord.into(), zoom))
        .collect::<Vec<_>>();
    let cells = draw_linestring(&[&ls], zoom, sampling_zoom, None)
        .into_iter()
        .map(|pw| pw.point)
        .collect::<HashSet<GPoint>>();

    let ground_truth_hashset = HashSet::from_iter(ground_truth_cells);
    let cells = cells
        .difference(&ground_truth_hashset)
        .cloned()
        .collect::<Vec<_>>();

    //for each non ground-truth cell, find taxicab distance to nearest ground-truth cell
    let cell_errors = cells
        .iter()
        .map(|cp| {
            (
                *cp,
                ground_truth_hashset
                    .iter()
                    .map(|gp| (gp.x - cp.x).abs() + (gp.y - cp.y).abs())
                    .min()
                    .unwrap_or(0),
            )
        })
        .collect::<Vec<_>>();
    cell_errors
}

//TODO: not really tested
/// error function by #cells generated via linestring with #cells generated via stop object
#[allow(unused_variables)]
fn stop_object_error<Dist: Fn(&PointM<4326>, &PointM<4326>) -> f64 + Send + Sync>(
    ls: &LineStringM<4326>,
    zoom: i32,
    conf: DbScanConf<Dist, 4326>,
) -> f64 {
    // let ls_count = draw_linestring(ls, zoom, zoom).len();
    // let stop_obj_count = todo!();

    let ls_cells = draw_linestring(&[ls], zoom, zoom, None)
        .into_iter()
        .collect::<HashSet<PointWTime>>();
    let stop_obj_cells: HashSet<PointWTime> = todo!();
    let diff = stop_obj_cells.difference(&ls_cells).count(); // man kan evt. måle afstand fra celle til nærmeste celle i `ls_cells` og bruge det som en fejlmetrik? på den måde
    diff as f64
    // stop_obj_count as f64 / ls_count as f64 //? evt sse,rmse hvis vi måler fejl over et helt datasæt
}

//TODO resume development on error measurement for stop-objects
#[allow(unused_variables)]
fn stop_object_error_cell_dist<Dist: Fn(&PointM<4326>, &PointM<4326>) -> f64 + Send + Sync>(
    ls: &LineStringM<4326>,
    zoom: i32,
    conf: DbScanConf<Dist, 4326>,
) -> f64 {
    let ls_cells = draw_linestring(&[ls], zoom, zoom, None)
        .into_iter()
        .collect::<HashSet<PointWTime>>();
    let stop_obj_cells: HashSet<PointWTime> = todo!();
    let diff = stop_obj_cells.difference(&ls_cells);
    let distances: f64 = diff
        .map(|c| {
            ls_cells
                .iter()
                .map(|lsc| {
                    let d = lsc.point - c.point;
                    let dist = ((d.x.pow(2) + d.y.pow(2)) as f64).sqrt();
                    dist
                })
                .min_by(|x, y| x.total_cmp(y))
                .unwrap_or(f64::INFINITY)
        })
        .sum();
    distances
}

#[cfg(test)]
mod test {
    use geo::{Coord, Point};
    use hex;
    use linesonmaps::types::coordm::CoordM;
    use linesonmaps::types::linestringm::LineStringM;
    use tilerizer::{Point as GPoint, draw_linestring};
    use wkb::reader::read_wkb;

    use crate::*;
    use tinymvt::webmercator::lnglat_to_zxy;

    #[test]
    fn it_works() {
        // POLYGON ((5.0 54.0, 10.0 54, 10.0 56, 5.0 56.0))
        let coords: Vec<CoordM<4326>> = [
            (5.0, 54.0, 0.0),
            (10.0, 54.0, 1.0),
            (10.0, 56.0, 2.0),
            (5.0, 56.0, 3.0),
        ]
        .map(|f| f.into())
        .to_vec(); // i.e. a square from (0,0) to (1,1)
        let ls = LineStringM::try_from(coords.clone()).unwrap();
        let cells = draw_linestring(&[&ls], 21, 21, None);
        assert!(cells.len() > 0);
        //TODO render same linestring, but with use of stop object (should cause an explosion in cell count)
    }

    #[test]
    fn cell_error() {
        const HEXSTRING: &str = include_str!("../../resources/mmsi245286000_surrogate4860673.txt");

        let bytea = hex::decode(HEXSTRING).unwrap();
        let wkb = read_wkb(&bytea).unwrap();
        let lsm = LineStringM::<4326>::try_from(wkb).unwrap();

        let e = line_error_from_ground_truth(&lsm, 19, 19);
        assert!(
            e.iter().all(|(_, d)| *d > 0),
            "no error value can be 0 since it only reports for non-ground truth cells"
        );
        // dbg!(&e);
        // assert!(false)
    }
    #[test]
    fn cell_error_euclidean() {
        const HEXSTRING: &str = include_str!("../../resources/mmsi245286000_surrogate4860673.txt");

        let bytea = hex::decode(HEXSTRING).unwrap();
        let wkb = read_wkb(&bytea).unwrap();
        let lsm = LineStringM::<4326>::try_from(wkb).unwrap();

        let e = line_error_from_ground_truth_geodesic(&lsm, 19, 19);
        assert!(
            e.iter().all(|(_, d)| *d > 0.0),
            "no error value can be 0 since it only reports for non-ground truth cells"
        );
        // dbg!(&e);
        // assert!(false)
    }
    #[test]
    fn cell_error_euclidean_typed_builder() {
        const HEXSTRING: &str = include_str!("../../resources/mmsi245286000_surrogate4860673.txt");

        let bytea = hex::decode(HEXSTRING).unwrap();
        let wkb = read_wkb(&bytea).unwrap();
        let lsm = LineStringM::<4326>::try_from(wkb).unwrap();

        let conf = ErrorMeasurementConf::builder()
            .method(ErrorMeasurementMethod::Geodesic)
            .zoom(19)
            .rendering_model(RenderingModel::Linestring)
            .build();

        assert_eq!(conf.sampling, None);
        let e = conf.measure_error(&lsm);
        // let e = line_error_from_ground_truth_geodesic(&lsm, 19, 19);
        assert!(
            e.iter().all(|(_, d)| *d > 0.0),
            "no error value can be 0 since it only reports for non-ground truth cells"
        );
        // dbg!(&e);
        // assert!(false)
    }
    #[test]
    fn grid_to_lng_lat_works() {
        let Point(Coord { x, y }) = Point(Coord { x: 45.0, y: 45.0 });

        let grid = lnglat_to_zxy(21, x, y);

        let Point(Coord { x: rx, y: ry }) = grid_centroid_to_lng_lat(
            GPoint {
                x: grid.1 as i32,
                y: grid.2 as i32,
            },
            grid.0,
        );
        // dbg!(&grid);
        // dbg!(ry);
        // assert!(false);
        assert!((x - rx).abs() < 1.0E-4, ":((( {0}", (x - rx).abs());
        assert!((y - ry).abs() < 1.0E-4, ":((( {0}", (y - ry).abs());
    }
}
