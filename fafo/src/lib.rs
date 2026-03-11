use std::collections::{HashMap, HashSet};
use std::f64;

use geo::algorithm::line_intersection::line_intersection;
use geo::line_measures::LengthMeasurable;
use geo::{
    Contains, Coord, Distance, GeoNum, Geodesic, Intersects, Line, LineIntersection, LineString,
    Point, Polygon,
};
use linesonmaps::types::{linestringm::LineStringM, pointm::PointM};
use tilerizer::{Point as GPoint, draw_2d_vessel, draw_linestring, point_to_grid};
use typed_builder::TypedBuilder;

pub type CellWithError = (xyzcell::Cell, f64);

pub mod xyzcell;

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
            p.coord == point_to_grid(f.coord.into(), self.zoom.into())
                || p.coord == point_to_grid(s.coord.into(), self.zoom.into())
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
        let interpolated_cells = cells.filter(|p| {
            p.coord == point_to_grid(f.coord.into(), self.zoom.into())
                || p.coord == point_to_grid(s.coord.into(), self.zoom.into())
        });

        interpolated_cells
            .map(|ic| self.length_of_line((f, s), &ic))
            .filter(|(c, e)| *e != 0_f64) //TODO this should not be necessary
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
        let poly = point_to_polygon(*gp);
        assert!(poly.intersects(&l), "polygon and line must intersect");
        let length = match poly.contains(&l) {
            true => line_contained_in_polygon(&l, &poly),
            false => {
                if poly.contains(&f) || poly.contains(&s) {
                    line_one_point_in_polygon(&l, &poly)
                } else {
                    line_no_end_point_in_polygon(&l, &poly)
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
                let first = ground_truth_to_cell_geodesic(f, gp, self.zoom);
                let second = ground_truth_to_cell_geodesic(s, gp, self.zoom);
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
                .map(|p| ground_truth_to_cell_geodesic(p, &gp, self.zoom))
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

/// deduplicates a nested list of cells (with their corresponding errors) by picking the minimum error value.
/// Useful after rendering and scoring all cells in a trajectory with [`ErrorMeasurementConf::cell_distance_to_ground_truth`] since it may yield multiple instances of the same cell
pub fn merge_cells<Cells: Iterator<Item = CellWithError>>(cells: Cells) -> Vec<CellWithError> {
    let s = cells.size_hint();
    let mut map = HashMap::<xyzcell::Cell, f64>::with_capacity(s.1.unwrap_or(s.0));

    cells.for_each(|(p, e)| {
        map.entry(p).and_modify(|v| *v = v.min(e)).or_insert(e);
    });

    map.into_iter().collect()
}

// implementation based on https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
pub fn grid_centroid_to_lng_lat(gp: xyzcell::Cell, _zoom: u8) -> Point<f64> {
    // seems to be close enough (not perfectly consistent with PostGIS)
    //TODO: might be incorrect since the original formula finds the nort-westernmost point
    let lon = ((0.5 + gp.coord.x as f64) / (2_f64.powi(gp.z as i32))) * 360_f64 - 180_f64;
    let lat = (f64::consts::PI
        - ((0.5 + gp.coord.y as f64) / 2_f64.powi(gp.z as i32) * 2_f64 * f64::consts::PI))
        .sinh()
        .atan()
        * (180_f64 / f64::consts::PI);
    Point(Coord { x: lon, y: lat })
}

/// i.e. used when `l` intersects `p` twice (without having either endpoint in `p`)
fn line_no_end_point_in_polygon(l: &Line, p: &Polygon) -> f64 {
    let ls = p.exterior().lines();
    let intersections = ls
        .filter_map(|pl| line_intersection(*l, pl)) // this contains 2 single point intersections OR one collinear intersection
        .map(|i| match i {
            LineIntersection::Collinear { intersection } => {
                vec![intersection.start, intersection.end]
            }
            LineIntersection::SinglePoint {
                intersection,
                is_proper: _, /* we dont care if it is proper */
            } => {
                vec![intersection]
            }
        })
        .flatten()
        .take(2)
        .collect::<Vec<_>>();
    debug_assert_eq!(
        intersections.len(),
        2,
        "function should only be called when there are 0 endpoints within the polygon"
    );

    Geodesic.distance(intersections[0].into(), intersections[1].into())
    // todo!()
}

fn line_one_point_in_polygon(l: &Line, p: &Polygon) -> f64 {
    let (f, s) = l.points();

    let a = [(f, p.contains(&f)), (s, p.contains(&s))]
        .into_iter()
        .filter(|(_, b)| *b)
        .map(|(q, _)| q)
        .next()
        .expect("atleast 1 point should be within the polygon");

    let lsr = p.exterior().lines();

    let intersection = lsr
        .into_iter()
        .filter_map(|pl| line_intersection(*l, pl))
        .map(|i| match i {
            LineIntersection::SinglePoint {
                intersection,
                is_proper: _,
            } => intersection,
            LineIntersection::Collinear { intersection } => {
                // one of the endpoints is equal to the endpoin in the polygon
                if intersection.start != a.0 {
                    intersection.start
                } else {
                    intersection.end
                }
            }
        })
        .next()
        .expect("should have exactly 1 intersecting point");

    assert!(a != intersection.into());
    Geodesic.distance(a, intersection.into())
}

fn line_contained_in_polygon(l: &Line, _p: &Polygon) -> f64 {
    l.length(&Geodesic)
}

fn point_to_polygon(c: xyzcell::Cell) -> Polygon {
    let lon = ((0.0 + c.coord.x as f64) / (2_f64.powi(c.z as i32))) * 360_f64 - 180_f64;
    let lon_1 = ((1.0 + c.coord.x as f64) / (2_f64.powi(c.z as i32))) * 360_f64 - 180_f64;

    let lat = (f64::consts::PI
        - ((0.0 + c.coord.y as f64) / 2_f64.powi(c.z as i32) * 2_f64 * f64::consts::PI))
        .sinh()
        .atan()
        * (180_f64 / f64::consts::PI);
    let lat_1 = (f64::consts::PI
        - ((1.0 + c.coord.y as f64) / 2_f64.powi(c.z as i32) * 2_f64 * f64::consts::PI))
        .sinh()
        .atan()
        * (180_f64 / f64::consts::PI);

    let ps = LineString::from(vec![
        (lon, lat_1),
        (lon, lat),
        (lon_1, lat),
        (lon_1, lat_1),
        (lon, lat_1), /* remember to close the polygon */
    ]); // TODO: ensure polygon is wound correctly // RE: seems to winding same as postgis now

    let poly = Polygon::new(ps, vec![]);
    //TODO: ensure this polygon is atleast somewhat consistent with postGIS

    poly
}

fn ground_truth_to_cell_geodesic<P: Into<Point<f64>>>(p: P, gp: &xyzcell::Cell, _zoom: u8) -> f64 {
    Geodesic.distance(grid_centroid_to_lng_lat(*gp, gp.z as u8), p.into())
}

#[cfg(test)]
mod test {
    use std::cell;

    use geo::{BooleanOps, Coord, GeodesicArea, Point};
    use hex;
    use linesonmaps::types::linestringm::LineStringM;
    use tilerizer::{Point as GPoint, draw_line, draw_linestring};
    use wkb::reader::read_wkb;

    use crate::xyzcell::Cell;
    use crate::*;
    use tinymvt::webmercator::lnglat_to_zxy;

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
                a: 10,
                b: 10,
                c: 10,
                d: 10,
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

        let grid = lnglat_to_zxy(21, x, y);

        let Point(Coord { x: rx, y: ry }) = grid_centroid_to_lng_lat(
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
    fn merge_cells_works() {
        let errors = vec![
            vec![
                (GPoint { x: 1, y: 1 }, 2.0),
                (GPoint { x: 2, y: 2 }, (10.0)),
            ],
            vec![(GPoint { x: 1, y: 1 }, (5.0)), (GPoint { x: 2, y: 2 }, 7.0)],
        ]
        .into_iter()
        .map(|v| {
            v.into_iter()
                .map(|(p, e)| (xyzcell::Cell { coord: p, z: 0 }, e))
        });

        let mut m = merge_cells(errors.into_iter().flatten());

        m.sort_by_key(|k| k.0.coord.x);

        assert_eq!(
            m,
            vec![
                (
                    Cell {
                        coord: GPoint { x: 1, y: 1 },
                        z: 0
                    },
                    2.0
                ),
                (
                    Cell {
                        coord: GPoint { x: 2, y: 2 },
                        z: 0
                    },
                    7.0
                )
            ]
        )
    }
    #[test]
    fn point_to_polygon_works() {
        let gp = GPoint { x: 10, y: 10 };
        let c = xyzcell::Cell { coord: gp, z: 10 }; // quadkey = 0000003030

        // testing in postgis seems to suggest that the difference in area is around 1E-6 square meters (at z=10)
        let polygon = point_to_polygon(c);
        // dbg!(polygon);
        // assert!(false);
    }

    #[test]
    fn point_to_polygon_sub_cell_contained() {
        // use geo::algorithm::bool_ops::xor
        let gp = GPoint { x: 10, y: 10 };
        let c = xyzcell::Cell { coord: gp, z: 10 }; // quadkey = 0000003030

        // testing in postgis seems to suggest that the difference in area is around 1E-6 square meters (at z=10)
        let polygon = point_to_polygon(c);

        let sub_poly = point_to_polygon(xyzcell::Cell {
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
        let polygon = point_to_polygon(c);

        let sub_poly = point_to_polygon(xyzcell::Cell {
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
}
