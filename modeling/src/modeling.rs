use std::ops::Div;

use chrono::{DateTime, Duration, Utc};
use geo::{Coord, Distance, InterpolatePoint, Vector2DOps, coord, point};
use geo_traits::{CoordTrait, LineTrait};
use geo_types::geometry::Triangle;
use linesonmaps::types::{linem::LineM, pointm::PointM};

pub struct LineTriangle<const CRS: u64> {
    pub triangle: Triangle,
    pub line: LineM<CRS>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
}

impl<const CRS: u64> LineTriangle<CRS> {
    pub fn point_occupation(&self, ba: f64, bb: f64, bc: f64) -> (DateTime<Utc>, DateTime<Utc>) {
        let probe_vec = probe_vector(&self.line, self.triangle, ba, bb, bc);

        let ratio = probe_ratio(
            probe_vec,
            self.line.end().x() - self.line.start().x(),
            self.line.end().y() - self.line.start().y(),
        );

        let probe_m = probe_timestamp(
            self.line.start().m,
            self.line.end().m - self.line.start().m,
            ratio,
        );

        let line_meters = meters_between_points(self.line.from, self.line.to);

        probe_occupation(
            probe_m,
            self.line.end().m - self.line.start().m,
            line_meters,
            self.a,
            self.b,
        )
    }
    pub fn distance_to_ais(&self,ba: f64, bb: f64, bc: f64) -> f64 {
        let probe_vec = probe_vector(&self.line, self.triangle, ba, bb, bc);
        let ratio = probe_ratio(
            probe_vec,
            self.line.end().x() - self.line.start().x(),
            self.line.end().y() - self.line.start().y(),
        );

        if ratio == 0. || ratio == 1. {
            return 0.
        }

        let line_meters = meters_between_points(self.line.from, self.line.to);
        dbg!(line_meters);

        if ratio <= 0.5 { // point closer to 'line.from'
            return line_meters*ratio
        }
        line_meters*(1.-ratio) // point closer to 'line.to' 
    }
}

pub fn barycentric_to_cartesian(triangle: Triangle, ba: f64, bb: f64, bc: f64) -> geo::Coord<f64> {
    coord![
        x: ba*triangle.0.x() + bb*triangle.1.x() + bc*triangle.2.x(),
        y: ba*triangle.0.y() + bb*triangle.1.y() + bc*triangle.2.y()
    ]
}

pub fn line_to_triangle_pair<const CRS: u64>(
    line: &LineM<CRS>,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
) -> (LineTriangle<CRS>, LineTriangle<CRS>) {
    let dx = line.end().x() - line.start().x();
    let dy = line.end().y() - line.start().y();

    //let vec_orth_c = vec![-dy, dx]; // orthogonal vector of the line, representative of c width
    //let vec_orth_d = vec![dy, -dx]; // orthogonal vector of the line, representative of d width

    let start_point_c = geo::algorithm::line_measures::metric_spaces::Geodesic
        .point_at_distance_between(
            point!(line.start().x_y()),
            point!(x: line.start().x()+(-dy), y: line.start().y()+(dx)),
            c,
        ); // Point 

    let start_point_d = geo::algorithm::line_measures::metric_spaces::Geodesic
        .point_at_distance_between(
            point!(line.start().x_y()),
            point!(x: line.start().x()+(dy), y: line.start().y()+(-dx)),
            d,
        );

    let end_point_c = geo::algorithm::line_measures::metric_spaces::Geodesic
        .point_at_distance_between(
            point!(line.end().x_y()),
            point!(x: line.end().x()+(-dy), y: line.end().y()+(dx)),
            c,
        );

    let end_point_d = geo::algorithm::line_measures::metric_spaces::Geodesic
        .point_at_distance_between(
            point!(line.end().x_y()),
            point!(x: line.end().x()+(dy), y: line.end().y()+(-dx)),
            d,
        );

    (
        LineTriangle {
            triangle: Triangle::new(start_point_c.0, start_point_d.0, end_point_c.0),
            line: *line,
            a,
            b,
            c,
            d,
        },
        LineTriangle {
            triangle: Triangle::new(start_point_d.0, end_point_c.0, end_point_d.0),
            line: *line,
            a,
            b,
            c,
            d,
        },
    )
}

pub fn probe_timestamp(start_m: f64, delta_m: f64, ratio: f64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_secs((delta_m * ratio) as i64 + start_m as i64)
        .expect("ratio er fucked")
}

pub fn probe_occupation(
    probe_m: DateTime<Utc>,
    delta_m: f64,
    line_meters: f64,
    a: f64,
    b: f64,
) -> (DateTime<Utc>, DateTime<Utc>) {
    if line_meters == 0. {
        return (probe_m, probe_m + Duration::seconds(delta_m as i64));
    }
    (
        probe_m - Duration::seconds((delta_m / line_meters * a) as i64), // formula: timestamp - 'how much earlier the ship arrived due to its length infront of sensor'
        probe_m + Duration::seconds((delta_m / line_meters * b) as i64), // formula: timestamp + 'how much longer did the ship stay due to its length behind the sensor'
    )
}

pub fn vector_length(x: f64, y: f64) -> f64 {
    f64::sqrt(f64::powi(x, 2) + f64::powi(y, 2))
}

pub fn vector_length2(x: f64, y: f64) -> f64 {
    f64::powi(x, 2) + f64::powi(y, 2)
}

pub fn meters_between_points<const CRS: u64>(origin: PointM<CRS>, destination: PointM<CRS>) -> f64 {
    geo::algorithm::line_measures::metric_spaces::Geodesic.distance(origin, destination)
}

pub fn probe_vector<const CRS: u64>(
    line: &LineM<CRS>,
    triangle: Triangle,
    ba: f64,
    bb: f64,
    bc: f64,
) -> Coord<f64> {
    let coord = barycentric_to_cartesian(triangle, ba, bb, bc);
    coord! {x: coord.x-line.start().x, y: coord.y-line.start().y}
}

// ratio of how far along the line the probe point is
pub fn probe_ratio(coord: Coord, dx: f64, dy: f64) -> f64 {
    if dx == 0. && dy == 0. {
        return 0.;
    }
    coord
        .dot_product(coord! {x: dx, y: dy})
        .div(vector_length2(dx, dy)) // length of the projected vector, formula: (|a_vec*b_vec|) / |a_vec| = |b_a_vec|

    //.div(vector_length(dx, dy)) // projection_length/length = ratio, small optimzation: (x/y)/y == x/(y^2)
    // Small optimization, note for future: this assumes (x/y)/y == x/y^2 (i belive this is true!)
}

#[cfg(test)]
mod tests {
    use crate::modeling::line_to_triangle_pair;
    use linesonmaps::types::{coordm::CoordM, linem::LineM};

    use super::*;

    #[test]
    fn half_way_test() {
        let start_m =
            DateTime::parse_from_str("2024-01-01 00:00:00 +0000", "%Y-%m-%d %H:%M:%S%.3f %z")
                .unwrap()
                .timestamp() as f64;
        let end_m =
            DateTime::parse_from_str("2024-01-01 00:02:00 +0000", "%Y-%m-%d %H:%M:%S%.3f %z")
                .unwrap()
                .timestamp() as f64;

        let coords: Vec<CoordM<4326>> = [(8.0, 56.0, start_m), (8.005, 56.0, end_m)]
            .map(|f| f.into())
            .to_vec();
        let line = LineM::<4326>::from((coords[0], coords[1]));

        let a = line_to_triangle_pair(&line, 1.0, 1.0, 10.0, 10.0);
        dbg!(a.0.distance_to_ais(1. / 2., 0., 1. / 2.)); // halfway
        dbg!(a.0.distance_to_ais(1. / 4., 1. / 2., 1. / 4.)); // quarter in
        
        assert_eq!(
            a.0.point_occupation(1. / 2., 0., 1. / 2.).0.timestamp() as f64 - start_m,
            (end_m - start_m) / 2.0
        )
    }

    #[test]
    fn no_distance_line() {
        let start_m =
            DateTime::parse_from_str("2024-01-01 00:00:00 +0000", "%Y-%m-%d %H:%M:%S%.3f %z")
                .unwrap()
                .timestamp() as f64;
        let end_m =
            DateTime::parse_from_str("2024-01-01 00:02:00 +0000", "%Y-%m-%d %H:%M:%S%.3f %z")
                .unwrap()
                .timestamp() as f64;

        let coords: Vec<CoordM<4326>> = [(8.0, 56.0, start_m), (8.0, 56.0, end_m)]
            .map(|f| f.into())
            .to_vec();
        let line = LineM::<4326>::from((coords[0], coords[1]));

        let a = line_to_triangle_pair(&line, 1., 1., 10.0, 10.0);
        assert_eq!(
            a.0.point_occupation(0., 1., 0.).0.timestamp(),
            start_m as i64
        );
        dbg!(a.0.distance_to_ais(0., 1., 0.));
        assert_eq!(a.0.point_occupation(0., 1., 0.).1.timestamp(), end_m as i64);
    }
}
