use std::cmp::min;
use std::collections::HashSet;
use std::f64;

use geo::{Coord, Distance, GeoNum, Geodesic, Point};
use linesonmaps::algo::stop_cluster::DbScanConf;
use linesonmaps::types::{linestringm::LineStringM, pointm::PointM};
use tilerizer::{Point as GPoint, PointWTime, draw_2d_vessel, draw_linestring, point_to_grid};
use typed_builder::TypedBuilder;

pub type CellWithError = (GPoint,f64);
#[derive(Debug, Clone, Copy, TypedBuilder)]
pub struct ErrorMeasurementConf {
    method: ErrorMeasurementMethod,
    // rendering_model: RenderingModel,
    zoom: u8,
    #[builder(default, setter(strip_option))]
    sampling: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorMeasurementMethod {
    /// Measures geodesic distance from cell centroid to nearest ground truth point.
    Geodesic,
    /// measures distance in terms of horizontal cell distance + vertical cell distance (i.e. an adjacent cell would have distance 1, while a diagonally adjacent cell would have distance 2)
    CellTaxicab,
}
#[derive(Debug, Clone, Copy)]
pub enum RenderingModel {
    /// Model ship movement as a linestring
    Linestring,
    /// Model ship movement as a moving polygon (i.e. linestring with width)
    TwoDimensional { a: u16, b: u16, c: u16, d: u16 }, // TODO this is bad since a single ErrorMeasurementConf no longer works across vessels with different dimensions
}

impl ErrorMeasurementConf {
    //TODO maybe there should be a function here for aggregating errors across multiple trajectories, but i do not know if it needs any more parameters
    /// Assigns error value to every rendered non ground-truth cell
    pub fn measure_error_entire_linestring(
        self,
        ls: &LineStringM<4326>,
        rendering_model: RenderingModel,
    ) -> Vec<CellWithError> {
        self.calculate_error(
            ls,
            &self
                .generate_cells(ls, rendering_model)
                .difference(&self.ground_truth_cells(ls))
                .cloned()
                .collect(),
        )
    }

    //TODO: this will not necesarilly give the same result at `measure_error`, since it only has two points-worth of context (in opposed to a linestring)
    pub fn cell_distance_to_ground_truth(
        &self,
        (f, s): (PointM<4326>, PointM<4326>),
        cells: &[GPoint],
    ) -> Vec<CellWithError> {
        let interpolated_cells = cells.iter().filter(|p| {
            **p == point_to_grid(f.coord.into(), self.zoom.into())
                || **p == point_to_grid(s.coord.into(), self.zoom.into())
        });

        interpolated_cells
            .map(|ic| self.cell_to_nearest_ground_truth((f, s), ic))
            .collect()
    }

    fn cell_to_nearest_ground_truth(
        &self,
        (f, s): (PointM<4326>, PointM<4326>),
        gp: &GPoint,
    ) -> CellWithError {
        match self.method {
            ErrorMeasurementMethod::CellTaxicab => {
                let (fc, sc) = (
                    point_to_grid(s.coord.into(), self.zoom.into()),
                    point_to_grid(f.coord.into(), self.zoom.into()),
                );
                let gp_to_fc = (fc.x - gp.x).abs() + (fc.y - gp.y).abs();
                let gp_to_sc = (sc.x - gp.x).abs() + (sc.y - gp.y).abs();
                if gp_to_fc < gp_to_sc {
                    (*gp, gp_to_fc as f64)
                } else {
                    (*gp, gp_to_sc as f64)
                }
            }
            ErrorMeasurementMethod::Geodesic => {
                let first = ground_truth_to_cell_geodesic(f, gp, self.zoom);
                let second = ground_truth_to_cell_geodesic(s, gp, self.zoom);
                let min = first.min(second);
                (*gp, min)
            }
        }

        // todo!()
    }
    fn calculate_error(
        &self,
        gt_ls: &LineStringM<4326>,
        cells: &HashSet<GPoint>,
    ) -> Vec<CellWithError> {
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
    fn generate_cells(
        &self,
        gt_ls: &LineStringM<4326>,
        rendering_model: RenderingModel,
    ) -> HashSet<GPoint> {
        let points = match rendering_model {
            RenderingModel::Linestring => draw_linestring(
                &[gt_ls.to_owned()],
                self.zoom.into(),
                self.sampling.unwrap_or(self.zoom).into(),
                None,
            ),
            RenderingModel::TwoDimensional { a, b, c, d } => draw_2d_vessel(
                &[gt_ls.to_owned()],
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
pub fn grid_centroid_to_lng_lat(gp: GPoint, zoom: u8) -> Point<f64> {
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

fn ground_truth_to_cell_geodesic<P: Into<Point<f64>>>(p: P, gp: &GPoint, zoom: u8) -> f64 {
    Geodesic.distance(grid_centroid_to_lng_lat(*gp, zoom), p.into())
}

//TODO: not really tested
/// error function by #cells generated via linestring with #cells generated via stop object
#[allow(unused_variables)]
fn stop_object_error<Dist: Fn(&PointM<4326>, &PointM<4326>) -> f64 + Send + Sync>(
    ls: &LineStringM<4326>,
    zoom: i32,
    conf: DbScanConf<Dist, 4326>,
) -> f64 {  
    let ls_cells = draw_linestring(&[ls.to_owned()], zoom, zoom, None)
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
    let ls_cells = draw_linestring(&[ls.to_owned()], zoom, zoom, None)
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
    use linesonmaps::types::linem::LineM;
    use linesonmaps::types::linestringm::LineStringM;
    use tilerizer::{Point as GPoint, PointWTime, draw_linestring};
    use wkb::reader::read_wkb;

    use crate::*;
    use tinymvt::webmercator::lnglat_to_zxy;

    #[test]
    #[ignore = "just foolin around"]
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
        let cells = draw_linestring(&[ls.to_owned()], 21, 21, None);
        assert!(cells.len() > 0);
        //TODO render same linestring, but with use of stop object (should cause an explosion in cell count)
    }

    #[test]
    fn cell_error() {
        const HEXSTRING: &str = include_str!("../../resources/mmsi245286000_surrogate4860673.txt");

        let bytea = hex::decode(HEXSTRING).unwrap();
        let wkb = read_wkb(&bytea).unwrap();
        let lsm = LineStringM::<4326>::try_from(wkb).unwrap();

        let conf = ErrorMeasurementConf::builder()
            .method(ErrorMeasurementMethod::CellTaxicab)
            .zoom(19)
            .build();

        assert_eq!(conf.sampling, None);
        let e = conf.measure_error_entire_linestring(&lsm, RenderingModel::Linestring);
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
            .build();

        assert_eq!(conf.sampling, None);
        let e = conf.measure_error_entire_linestring(&lsm, RenderingModel::Linestring);
        assert!(
            e.iter().all(|(_, d)| *d > 0.0),
            "no error value can be 0 since it only reports for non-ground truth cells"
        );
        // dbg!(&e);
        // assert!(false)
    }
    #[test]
    fn cell_error_euclidean_2d_typed_builder() {
        const HEXSTRING: &str = include_str!("../../resources/mmsi245286000_surrogate4860673.txt");

        let bytea = hex::decode(HEXSTRING).unwrap();
        let wkb = read_wkb(&bytea).unwrap();
        let lsm = LineStringM::<4326>::try_from(wkb).unwrap();

        let ls_conf = ErrorMeasurementConf::builder()
            .method(ErrorMeasurementMethod::Geodesic)
            .zoom(19)
            .build();
        let conf = ErrorMeasurementConf::builder()
            .method(ErrorMeasurementMethod::Geodesic)
            .zoom(19)
            .build();

        assert_eq!(conf.sampling, None);
        let e = conf.measure_error_entire_linestring(
            &lsm,
            RenderingModel::TwoDimensional {
                a: 10,
                b: 10,
                c: 10,
                d: 10,
            },
        );
        let ls_e = ls_conf.measure_error_entire_linestring(&lsm, RenderingModel::Linestring);
        // let e = line_error_from_ground_truth_geodesic(&lsm, 19, 19);
        assert!(
            e.iter().all(|(_, d)| *d > 0.0),
            "no error value can be 0 since it only reports for non-ground truth cells"
        );
        assert!(
            ls_e.len() < e.len(),
            "2d renderer should render at least as many points"
        );
        // dbg!(&e);
        // assert!(false)
    }
    #[test]
    fn cell_error_euclidean_super_sampling_typed_builder() {
        const HEXSTRING: &str = include_str!("../../resources/mmsi245286000_surrogate4860673.txt");

        let bytea = hex::decode(HEXSTRING).unwrap();
        let wkb = read_wkb(&bytea).unwrap();
        let lsm = LineStringM::<4326>::try_from(wkb).unwrap();

        let ss_conf = ErrorMeasurementConf::builder()
            .method(ErrorMeasurementMethod::Geodesic)
            .zoom(19)
            .sampling(21)
            .build();
        let conf = ErrorMeasurementConf::builder()
            .method(ErrorMeasurementMethod::Geodesic)
            .zoom(19)
            .build();

        assert_eq!(conf.sampling, None);
        let e = conf.measure_error_entire_linestring(&lsm, RenderingModel::Linestring);
        let ss_e = ss_conf.measure_error_entire_linestring(&lsm, RenderingModel::Linestring);
        assert!(
            e.iter().all(|(_, d)| *d > 0.0),
            "no error value can be 0 since it only reports for non-ground truth cells"
        );
        assert!(
            ss_e.len() > e.len(),
            "super sampling should result in at least as many cells"
        );
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

    #[test]
    fn cell_error_from_single_line() {
        const HEXSTRING: &str = include_str!("../../resources/mmsi245286000_surrogate4860673.txt");

        let bytea = hex::decode(HEXSTRING).unwrap();
        let wkb = read_wkb(&bytea).unwrap();
        let lsm = LineStringM::<4326>::try_from(wkb).unwrap();

        let conf = ErrorMeasurementConf::builder()
            .method(ErrorMeasurementMethod::Geodesic)
            .zoom(19)
            .build();

        let lines_to_cells = lsm
            .lines()
            .map(|l| LineStringM::try_from(l).unwrap())
            .map(|ls| {
                (
                    (PointM::from(ls.0[0]), PointM::from(ls.0[1])),
                    draw_linestring(
                        &[ls.clone()],
                        conf.zoom.into(),
                        conf.sampling.unwrap_or(conf.zoom).into(),
                        None,
                    )
                    .iter()
                    .map(|gpwt| gpwt.point)
                    .collect::<Vec<_>>(),
                )
            });

        let mut errors = lines_to_cells.map(|(ps, cs)| conf.cell_distance_to_ground_truth(ps, &cs));

        assert!(
            errors.all(|v| v.iter().all(|(_, e)| *e > 0.0)),
            "Every non ground-truth cell should have at least some error"
        );
    }
}
