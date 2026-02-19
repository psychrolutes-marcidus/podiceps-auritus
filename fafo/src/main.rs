#![allow(dead_code)]
use std::collections::HashSet;

use linesonmaps::algo::stop_cluster::DbScanConf;
use linesonmaps::types::{linem::LineM, linestringm::LineStringM, pointm::PointM};
use tilerizer::{Point, PointWTime, draw_linestring, point_to_grid};

fn main() {
    println!("Hello, world!");
}

/* TODO:
    - mvt grid to polygon<4326>
    
*/

fn line_error_from_ground_truth(
    ls: &LineStringM<4326>,
    zoom: i32,
    sampling_zoom: i32,
) -> Vec<(Point, i32)> {
    let ground_truth = ls.points();
    let ground_truth_cells = ground_truth
        .map(|p| point_to_grid(p.coord.into(), zoom))
        .collect::<Vec<_>>();
    // .collect::<HashSet<Point>>();
    let cells = draw_linestring(&[&ls], zoom, sampling_zoom, None)
        .into_iter()
        .map(|pw| pw.point)
        .collect::<HashSet<Point>>();

    let ground_truth_hashset = HashSet::from_iter(ground_truth_cells);
    let cells = cells
        .difference(&ground_truth_hashset)
        .cloned()
        .collect::<Vec<_>>();
    // for each non-ground truth cell, find euclidian distance to nearest ground-truth cell
    // let cells_with_distances = cells.into_iter().map(|cp| {ground_truth_cells});

    let a = cells
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
    a
}

/// error function by #cells generated via linestring with #cells generated via stop object
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

mod test {
    // #![allow(dead_code)]
    use hex;
    use linesonmaps::types::coordm::CoordM;
    use linesonmaps::types::linestringm::LineStringM;
    use linesonmaps::types::*;
    use tilerizer::draw_linestring;
    use wkb::reader::read_wkb;

    use crate::line_error_from_ground_truth;

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
        assert!(e.iter().all(|(_,d)| *d>0), "no error value can be 0 since it only reports for non-ground truth cells");
        dbg!(&e);
        assert!(false)
    }
}
