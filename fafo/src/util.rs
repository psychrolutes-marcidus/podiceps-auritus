use geo::{
    Contains, Coord, Distance, Geodesic, Line, LineIntersection, LineString, Point, Polygon,
    line_intersection::line_intersection, line_measures::LengthMeasurable,
};

use crate::xyzcell;

use std::collections::HashMap;
use std::f64;

use super::CellWithError;



// implementation based on https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
pub fn grid_centroid_to_lon_lat(gp: xyzcell::Cell, _zoom: u8) -> Point<f64> {
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
pub(crate) fn line_no_end_point_in_polygon(l: &Line, p: &Polygon) -> f64 {
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

pub(crate) fn line_one_point_in_polygon(l: &Line, p: &Polygon) -> f64 {
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

pub(crate) fn line_contained_in_polygon(l: &Line, _p: &Polygon) -> f64 {
    l.length(&Geodesic)
}

pub(crate) fn point_to_polygon(c: xyzcell::Cell) -> Polygon {
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

pub(crate) fn ground_truth_to_cell_geodesic<P: Into<Point<f64>>>(
    p: P,
    gp: &xyzcell::Cell,
    _zoom: u8,
) -> f64 {
    Geodesic.distance(grid_centroid_to_lon_lat(*gp, gp.z as u8), p.into())
}
