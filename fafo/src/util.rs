use crate::xyzcell;
use geo::Covers;
use geo::{
    Contains, Coord, Distance, Geodesic, Line, LineIntersection, LineString, Point, Polygon,
    line_intersection::line_intersection, line_measures::LengthMeasurable,
};
use std::f64;

// implementation based on https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
pub fn grid_centroid_to_lon_lat(gp: xyzcell::Cell, _zoom: u8) -> Point<f64> {
    // seems to be close enough (not perfectly consistent with PostGIS)
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
        .filter_map(|pl| line_intersection(*l, pl)) // this contains 2 single point intersections XOR 1 collinear intersection
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
    assert_eq!(
        intersections.len(),
        2,
        "function should only be called when there are 0 endpoints within the polygon"
    );

    Geodesic.distance(intersections[0].into(), intersections[1].into())
}

/// when either of the endpoints are within the polygon
pub(crate) fn line_one_point_in_polygon(l: &Line, p: &Polygon) -> f64 {
    let (f, s) = l.points();

    let point_that_are_covered = [(f, p.covers(&f)), (s, p.covers(&s))]
        .into_iter()
        .filter(|(_, b)| *b)
        .map(|(q, _)| q);
    let c = point_that_are_covered.clone();
    let a = point_that_are_covered
        .take(1)
        .next()
        .expect("atleast 1 point should be within the polygon");
    assert_eq!(
        c.count(),
        1,
        "this function only works when one endpoint is covered by the input polygon"
    );
    // .next()
    // .expect("atleast 1 point should be within the polygon");

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

    // assert_ne!(a, intersection.into());
    Geodesic.distance(a, intersection.into())
}

/// When entire line is contained within polygon
pub(crate) fn line_contained_in_polygon(l: &Line, p: &Polygon) -> f64 {
    assert!(p.covers(l), "line should be covered by polygon");
    l.length(&Geodesic)
}

pub(crate) fn cell_to_polygon(c: xyzcell::Cell) -> Polygon {
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
        (lon, lat_1),   // NW
        (lon, lat),     // SW
        (lon_1, lat),   // SE
        (lon_1, lat_1), // NE
        (lon, lat_1),   // NW /* remember to close the polygon */
    ]);

    let poly = Polygon::new(ps, vec![]);

    poly
}

pub(crate) fn ground_truth_to_cell_centroid_geodesic<P: Into<Point<f64>>>(
    p: P,
    gp: &xyzcell::Cell,
    _zoom: u8,
) -> f64 {
    Geodesic.distance(grid_centroid_to_lon_lat(*gp, gp.z as u8), p.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::coord;
    use geo::geometry::Rect;
    #[test]
    fn line_one_point_in_polygon_works() {
        let corner = coord! {x:10.,y:20.};
        let p = Rect::new(corner, coord! {x:30.,y:10.}).to_polygon();
        assert_eq!(p.exterior().lines().count(), 4);
        let l = Line::new(coord! {x:10.,y:30.}, corner); // line with startpoint outside polygon and endpoint in polygon corner
        let _length = line_one_point_in_polygon(&l, &p); // ensures that assertion is not violated
    }
}
