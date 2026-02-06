use std::collections::HashSet;

use linesonmaps::algo::stop_cluster::DbScanConf;
use linesonmaps::types::linestringm::LineStringM;
use linesonmaps::types::pointm::PointM;
use tilerizer::{PointWTime, draw_linestring};

fn main() {
    println!("Hello, world!");
}

/* idéer til fejlmetrikker ift. stop objetker
    - antal celler renderedet via stop objekts over antal celler renderet via punkter fra stop objekt (stop objekt skulle gerne rendere mindst lige så mange celler i alle tilfælde)

*/

/// error function by #cells generated via linestring with #cells generated via stop object
fn stop_object_error<Dist: Fn(&PointM<4326>, &PointM<4326>) -> f64 + Send + Sync>(
    ls: &LineStringM<4326>,
    zoom: i32,
    conf: DbScanConf<Dist, 4326>,
) -> f64 {
    // let ls_count = draw_linestring(ls, zoom, zoom).len();
    // let stop_obj_count = todo!();

    let ls_cells = draw_linestring(ls, zoom, zoom)
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
    let ls_cells = draw_linestring(ls, zoom, zoom)
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
    use linesonmaps::types::coordm::CoordM;
    use linesonmaps::types::linestringm::LineStringM;
    use linesonmaps::types::*;
    use tilerizer::draw_linestring;

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
        let cells = draw_linestring(&ls, 21, 21);
        assert!(cells.len() > 0);
        //TODO render same linestring, but with use of stop object (should cause an explosion in cell count)
    }
}
