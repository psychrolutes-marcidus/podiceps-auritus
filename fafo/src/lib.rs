use std::collections::HashSet;
use std::f64;

use geo::{
    BooleanOps as _, ConvexHull, Covers, GeoNum, GeodesicArea, Intersects, Line, Point, Polygon,
    Relate,
};
use linesonmaps::types::{linestringm::LineStringM, pointm::PointM};
use modeling::modeling::LineTriangle;
use tilerizer::{draw_2d_vessel, draw_linestring, point_to_grid};
use typed_builder::TypedBuilder;
pub mod confidence;
pub mod util;
pub mod xyzcell;

pub type CellWithError = (xyzcell::Cell, f64);

#[derive(Debug, Clone, Copy, TypedBuilder)]
pub struct ErrorMeasurementConf {
    method: ErrorMeasurementMethod,
    // rendering_model: RenderingModel,
    zoom: u8,
    #[builder(default, setter(strip_option))]
    sampling: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorMeasurementMethod {
    /// Measures geodesic distance from cell centroid to nearest ground truth point.
    Geodesic,
    /// measures distance in terms of horizontal cell distance + vertical cell distance (i.e. an adjacent cell would have distance 1, while a diagonally adjacent cell would have distance 2)
    CellTaxicab,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderingModel {
    /// Model ship movement as a linestring
    Linestring,
    /// Model ship movement as a moving polygon (i.e. linestring with width)
    TwoDimensional { a: u16, b: u16, c: u16, d: u16 },
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

    //TODO: this will not necessarily give the same result at [`ErrorMeasurementConf::measure_error_entire_linestring`], since it only has two points-worth of context (in opposed to a linestring)
    pub fn cell_distance_to_ground_truth<Cells: Iterator<Item = xyzcell::Cell>>(
        &self,
        (f, s): (PointM<4326>, PointM<4326>),
        cells: Cells,
    ) -> Vec<CellWithError> {
        let interpolated_cells = cells.filter(|p| {
            !(p.coord == point_to_grid(f.coord.into(), self.zoom.into())
                || p.coord == point_to_grid(s.coord.into(), self.zoom.into()))
        });

        interpolated_cells
            .map(|ic| self.cell_to_nearest_ground_truth((f, s), &ic))
            .collect()
    }
    /// only appropiate using linestring (not polygon) renderer
    pub fn length_of_line_in_cells<Cells: Iterator<Item = xyzcell::Cell>>(
        &self,
        (f, s): (PointM<4326>, PointM<4326>),
        cells: Cells,
    ) -> Vec<CellWithError> {
        // let interpolated_cells = cells.filter(|p| {
        //     p.coord == point_to_grid(f.coord.into(), self.zoom.into())
        //         || p.coord == point_to_grid(s.coord.into(), self.zoom.into())
        // });
        // interpolated_cells

        cells
            .map(|ic| self.length_of_line((f, s), &ic))
            .filter(|(_c, e)| *e != 0_f64)
            .collect()
    }
    /// Should be called on the portion of a trajectory corresponding to a stop object
    pub fn stop_object_cell_to_ground_truth<Cells: Iterator<Item = xyzcell::Cell>>(
        &self,
        ground_truth: &[PointM<4326>],
        stop_object_cells: Cells,
    ) -> Vec<CellWithError> {
        stop_object_cells
            .map(|c| {
                (
                    c,
                    self.cell_to_nearest_point(ground_truth.iter().copied(), &c),
                )
            })
            .collect()
    }

    fn length_of_line(
        &self,
        (f, s): (PointM<4326>, PointM<4326>),
        gp: &xyzcell::Cell,
    ) -> CellWithError {
        let f = Point::new(f.coord.x, f.coord.y);
        let s = Point::new(s.coord.x, s.coord.y);
        let l = Line::new(f, s);
        let poly = util::cell_to_polygon(*gp);
        let mat_start = poly.relate(&f);
        let mat_end = poly.relate(&s);
        let length = match mat_start.is_covers() && mat_end.is_covers() /* if start and end is covered by p, then whole line must be covered as well */ {
            true => util::line_contained_in_polygon(&l, &poly),
            false => {
                if mat_start.is_disjoint() && mat_end.is_disjoint() {
                    0_f64
                } else if mat_start.is_covers() || mat_end.is_covers() {
                    util::line_one_point_in_polygon(&l, &poly)
                } else {
                    util::line_no_end_point_in_polygon(&l, &poly)
                }
            }
        };
        (*gp, length)
    }

    fn cell_to_nearest_ground_truth(
        &self,
        (f, s): (PointM<4326>, PointM<4326>),
        gp: &xyzcell::Cell,
    ) -> CellWithError {
        match self.method {
            ErrorMeasurementMethod::CellTaxicab => {
                let (fc, sc) = (
                    point_to_grid(s.coord.into(), self.zoom.into()),
                    point_to_grid(f.coord.into(), self.zoom.into()),
                );
                let gp_to_fc = (fc.x - gp.coord.x).abs() + (fc.y - gp.coord.y).abs();
                let gp_to_sc = (sc.x - gp.coord.x).abs() + (sc.y - gp.coord.y).abs();
                if gp_to_fc < gp_to_sc {
                    (*gp, gp_to_fc as f64)
                } else {
                    (*gp, gp_to_sc as f64)
                }
            }
            ErrorMeasurementMethod::Geodesic => {
                let first = util::ground_truth_to_cell_centroid_geodesic(f, gp, self.zoom);
                let second = util::ground_truth_to_cell_centroid_geodesic(s, gp, self.zoom);
                let min = first.min(second);
                (*gp, min)
            }
        }
    }
    fn calculate_error(
        &self,
        gt_ls: &LineStringM<4326>,
        cells: &HashSet<xyzcell::Cell>,
    ) -> Vec<CellWithError> {
        let ground_truth_cells = self.ground_truth_cells(&gt_ls);
        debug_assert!(
            ground_truth_cells.intersection(cells).count() == 0,
            "cells should be disjoint with ground-truth cells"
        );
        cells
            .iter()
            .map(|c| (*c, self.cell_to_nearest_point(gt_ls.points(), c)))
            .collect()
    }
    fn cell_to_nearest_point<P: Iterator<Item = PointM<4326>>>(
        &self,
        gt: P,
        gp: &xyzcell::Cell,
    ) -> f64 {
        match self.method {
            ErrorMeasurementMethod::CellTaxicab => gt
                .map(|p| point_to_grid(p.coord.into(), self.zoom.into()))
                .map(|c| (c.x - gp.coord.x).abs() + (c.y - gp.coord.y).abs())
                .min_by(|x, y| x.total_cmp(y))
                .unwrap_or(0) as f64,
            ErrorMeasurementMethod::Geodesic => gt
                .map(|p| util::ground_truth_to_cell_centroid_geodesic(p, &gp, self.zoom))
                .min_by(|x, y| x.total_cmp(y))
                .unwrap_or(0.0),
        }
    }
    fn generate_cells(
        &self,
        gt_ls: &LineStringM<4326>,
        rendering_model: RenderingModel,
    ) -> HashSet<xyzcell::Cell> {
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
            .map(|pw| xyzcell::Cell {
                coord: pw.point,
                z: pw.z as u32,
            })
            .collect::<HashSet<xyzcell::Cell>>()
            .difference(&self.ground_truth_cells(gt_ls))
            .cloned()
            .collect()
    }
    fn ground_truth_cells(&self, gt_ls: &LineStringM<4326>) -> HashSet<xyzcell::Cell> {
        gt_ls
            .points()
            .map(|p| xyzcell::Cell {
                coord: point_to_grid(p.coord.into(), self.zoom.into()),
                z: self.zoom.into(),
            })
            .collect()
    }
}

//TODO: maybe i should delete
pub fn cell_relative_coverage_by_polygon(
    rectangle: (&LineTriangle<4326>, &LineTriangle<4326>),
    gp: &xyzcell::Cell,
) -> f64 {
    //TODO assert that triangles touch
    let mut mlp = rectangle
        .0
        .triangle
        .to_polygon()
        .union(&rectangle.1.triangle.to_polygon());
    debug_assert_eq!(
        mlp.convex_hull().geodesic_area_signed(),
        mlp.0[0].geodesic_area_signed(),
        "input triangles are not perfectly adjacent"
    );
    let polygon = mlp
        .0
        .pop()
        .expect("union operation should yield a single polygon");
    let grid_poly = util::cell_to_polygon(*gp);

    let difference = grid_poly.intersection(&polygon);
    difference.geodesic_area_unsigned() / grid_poly.geodesic_area_unsigned()
}
pub fn cells_relative_coverage_by_polygon<Cells: Iterator<Item = xyzcell::Cell>>(
    rectangle: (&LineTriangle<4326>, &LineTriangle<4326>),
    gp: Cells,
) -> Vec<CellWithError> {
    let mut mlp = rectangle
        .0
        .triangle
        .to_polygon()
        .union(&rectangle.1.triangle.to_polygon());
    debug_assert_eq!(
        mlp.convex_hull().geodesic_area_signed(),
        mlp.0[0].geodesic_area_signed(),
        "input triangles are not perfectly adjacent"
    );
    let polygon = mlp
        .0
        .pop()
        .expect("union operation should yield a single polygon");
    gp.map(|c| (c, util::cell_to_polygon(c)))
        .map(|(c, gpoly)| {
            (
                c,
                gpoly.intersection(&polygon).geodesic_area_unsigned()
                    / gpoly.geodesic_area_unsigned(),
            )
        })
        .collect()
}
#[cfg(test)]
mod test {

    use geo::{BooleanOps, Coord, GeodesicArea, Point};
    use hex;
    use linesonmaps::types::linestringm::LineStringM;
    use modeling::modeling::line_to_triangle_pair;
    use tilerizer::tile3d::draw_triangle;
    use tilerizer::{Point as GPoint, draw_linestring};
    use wkb::reader::read_wkb;

    use crate::xyzcell::Cell;
    use crate::*;
    use tinymvt::webmercator::lnglat_to_zxy as lonlat_to_zxy;

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
                a: 100,
                b: 100,
                c: 100,
                d: 100,
            },
        );
        let ls_e = ls_conf.measure_error_entire_linestring(&lsm, RenderingModel::Linestring);
        assert!(
            e.iter().all(|(_, d)| *d > 0.0),
            "no error value can be 0 since it only reports for non-ground truth cells"
        );
        assert!(
            ls_e.len() < e.len(),
            "2d renderer should render at least as many points"
        );
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

        let grid = lonlat_to_zxy(21, x, y);

        let Point(Coord { x: rx, y: ry }) = util::grid_centroid_to_lon_lat(
            xyzcell::Cell {
                coord: GPoint {
                    x: grid.1 as i32,
                    y: grid.2 as i32,
                },
                z: grid.0.into(),
            },
            grid.0,
        );
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
                    .map(|gpwt| xyzcell::Cell {
                        coord: gpwt.point,
                        z: gpwt.z as u32,
                    })
                    .collect::<Vec<_>>(),
                )
            });

        let mut errors =
            lines_to_cells.map(|(ps, cs)| conf.cell_distance_to_ground_truth(ps, cs.into_iter()));

        assert!(
            errors.all(|v| v.iter().all(|(_, e)| *e > 0.0)),
            "Every non ground-truth cell should have at least some error"
        );
    }
    #[test]
    fn point_to_polygon_works() {
        let gp = GPoint { x: 10, y: 10 };
        let c = xyzcell::Cell { coord: gp, z: 10 }; // quadkey = 0000003030

        // testing in postgis seems to suggest that the difference in area is around 1E-6 square meters (at z=10)
        let polygon = util::cell_to_polygon(c);
        // dbg!(polygon);
        // assert!(false);
    }

    #[test]
    fn point_to_polygon_sub_cell_contained() {
        // use geo::algorithm::bool_ops::xor
        let gp = GPoint { x: 10, y: 10 };
        let c = xyzcell::Cell { coord: gp, z: 10 }; // quadkey = 0000003030

        // testing in postgis seems to suggest that the difference in area is around 1E-6 square meters (at z=10)
        let polygon = util::cell_to_polygon(c);

        let sub_poly = util::cell_to_polygon(xyzcell::Cell {
            coord: GPoint {
                x: gp.x * 2,
                y: gp.y * 2,
            },
            z: c.z + 1,
        });

        // let mp = polygon.xor(&sub_poly); // this is stupid
        let mp = sub_poly.difference(&polygon); // this is smart hehe
        let a = mp.geodesic_area_unsigned();
        dbg!(a);
        assert!(a == 0_f64); // dunno if this is the case for every sub-cell
    }
    #[test]
    fn point_to_polygon_sub_cell_contained_finer_resolution() {
        // use geo::algorithm::bool_ops::xor
        let gp = GPoint {
            x: 10 * 11 * 2,
            y: 10 * 11 * 2,
        };
        let c = xyzcell::Cell { coord: gp, z: 21 }; // quadkey = 0000003030

        // testing in postgis seems to suggest that the difference in area is around 1E-6 square meters (at z=10)
        let polygon = util::cell_to_polygon(c);

        let sub_poly = util::cell_to_polygon(xyzcell::Cell {
            coord: GPoint {
                x: gp.x * 2,
                y: gp.y * 2,
            },
            z: c.z + 1,
        });

        // let mp = polygon.xor(&sub_poly); // this is stupid
        let mp = sub_poly.difference(&polygon); // this is smart hehe
        let a = mp.geodesic_area_unsigned();
        dbg!(a);
        assert!(a == 0_f64); // dunno if this is the case for every sub-cell
    }
    #[test]
    fn length_of_line_works() {
        const HEXSTRING: &str = include_str!("../../resources/mmsi245286000_surrogate4860673.txt");

        let bytea = hex::decode(HEXSTRING).unwrap();
        let wkb = read_wkb(&bytea).unwrap();
        let lsm = LineStringM::<4326>::try_from(wkb).unwrap();
        assert!(lsm.points().count() != 0);
        let conf = ErrorMeasurementConf::builder()
            .method(ErrorMeasurementMethod::Geodesic)
            .zoom(21)
            .build();

        let cells = draw_linestring(&[lsm.clone()], 21, 21, None)
            .iter()
            .map(|pw| xyzcell::Cell {
                coord: pw.point,
                z: pw.z as u32,
            })
            .collect::<Vec<_>>();
        // assert!(cells.len() > 0);

        let (l, cells_length): (Vec<_>, Vec<_>) = lsm
            .lines()
            .map(|lm| {
                (
                    lm,
                    conf.length_of_line_in_cells((lm.from, lm.to), cells.iter().copied()),
                )
            })
            .unzip();
        let cells_length = cells_length.into_iter().flatten().collect::<Vec<_>>();
        // assert!(cells_length.len() > 0);
        let b = l
            .iter()
            .zip(cells_length.iter())
            .filter(|(_, (c, e))| *e == 0_f64)
            .map(|(l, ce)| (Line::new(l.from.coord, l.to.coord), ce))
            .collect::<Vec<_>>();
        // dbg!(b);
        // assert!(false);
        //length_of_line_in_cells
        // let e = conf.measure_error_entire_linestring(&lsm, RenderingModel::Linestring);
        // dbg!(&cells_length);
        assert!(
            cells_length.iter().all(|(_, d)| *d > 0.0),
            "all lenghts should be greater than 0 (assuming linestrings don't have duplicate points"
        );
        // assert!(false)
    }
    #[test]
    fn realtive_area_works() {
        const HEXSTRING: &str = include_str!("../../resources/mmsi245286000_surrogate4860673.txt");

        let bytea = hex::decode(HEXSTRING).unwrap();
        let wkb = read_wkb(&bytea).unwrap();
        let lsm = LineStringM::<4326>::try_from(wkb)
            .unwrap()
            .lines()
            .next()
            .unwrap();
        let triangles = line_to_triangle_pair(&lsm, 100_f64, 100_f64, 100_f64, 100_f64);

        let cells = draw_triangle(triangles.0.triangle, 21)
            .into_iter()
            .chain(draw_triangle(triangles.1.triangle, 21))
            .map(|pw| xyzcell::Cell::from(pw));
        let frac = cells_relative_coverage_by_polygon((&triangles.0, &triangles.1), cells);
        dbg!(&frac);
        assert!(frac.iter().all(|cw| cw.1 >= 0_f64 && cw.1 <= 1_f64))
    }
}
