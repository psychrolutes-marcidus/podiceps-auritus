use std::ops::Div;

use chrono::{DateTime, Duration, TimeDelta, Utc};
use geo::{Coord, Distance, InterpolatePoint, Vector2DOps, coord, point};
use geo_traits::{CoordTrait, LineTrait};
use geo_types::geometry::{Line, Triangle};
use linesonmaps::types::{coordm::CoordM, linem::LineM, pointm::PointM};

#[derive(Debug)]
pub struct LineTriangle<const CRS: u64> {
    pub triangle: Triangle,
    pub line: LineM<CRS>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub ba_line: Line,
}

impl<const CRS: u64> LineTriangle<CRS> {
    pub fn point_occupation(&self, ba: f64, bb: f64, bc: f64) -> (DateTime<Utc>, DateTime<Utc>) {
        let probe_vec = probe_vector(&self.ba_line, self.triangle, ba, bb, bc);

        let ratio = probe_ratio(
            probe_vec,
            self.ba_line.end.x() - self.ba_line.start.x(),
            self.ba_line.end.y() - self.ba_line.start.y(),
        );

        let line_meters = meters_between_points(self.line.from, self.line.to);

        let b_start_m = DateTime::<Utc>::from_timestamp_secs(self.line.start().m as i64)
            .expect("error ;(")
            - vessel_speed(&self.line, self.b, line_meters); // The time when a ship transponder reached the start edge of its ship polygon: polygon.line.from.m = line.from.m - (how long it takes the ship to travel 'b' distance)

        let a_end_m = DateTime::<Utc>::from_timestamp_secs(self.line.end().m as i64)
            .expect("error ;(")
            + vessel_speed(&self.line, self.a, line_meters); // The time when a ship transponder reached the end edge of its ship polygon: polygon.line.to.m = line.to.m + (how long it takes the ship to travel 'a' distance)

        let probe_m = probe_timestamp(
            b_start_m.timestamp() as f64,
            (a_end_m.timestamp() - b_start_m.timestamp()) as f64,
            ratio,
        );

        let ba_meters = meters_between_points(
            PointM::<4326> {
                coord: CoordM::<4326> {
                    x: self.ba_line.start.x,
                    y: self.ba_line.start.y,
                    m: b_start_m.timestamp() as f64,
                },
            },
            PointM::<4326> {
                coord: CoordM::<4326> {
                    x: self.ba_line.end.x,
                    y: self.ba_line.end.y,
                    m: a_end_m.timestamp() as f64,
                },
            },
        );

        probe_occupation(
            probe_m,
            (a_end_m.timestamp() - b_start_m.timestamp()) as f64,
            ba_meters,
            self.a,
            self.b,
        )
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

    let b_start = geo::algorithm::line_measures::metric_spaces::Geodesic.point_at_distance_between(
        point!(line.start().x_y()),
        point!(x: line.start().x()-dx, y: line.start().y()-dy),
        b,
    );

    let a_end = geo::algorithm::line_measures::metric_spaces::Geodesic.point_at_distance_between(
        point!(line.end().x_y()),
        point!(x: line.end().x()+dx, y: line.end().y()+dy),
        a,
    );

    let start_point_c = geo::algorithm::line_measures::metric_spaces::Geodesic
        .point_at_distance_between(
            point!(b_start.x_y()),
            point!(x: b_start.x()+(-dy), y: b_start.y()+(dx)),
            c,
        ); // Point 

    let start_point_d = geo::algorithm::line_measures::metric_spaces::Geodesic
        .point_at_distance_between(
            point!(b_start.x_y()),
            point!(x: b_start.x()+(dy), y: b_start.y()+(-dx)),
            d,
        );

    let end_point_c = geo::algorithm::line_measures::metric_spaces::Geodesic
        .point_at_distance_between(
            point!(a_end.x_y()),
            point!(x: a_end.x()+(-dy), y: a_end.y()+(dx)),
            c,
        );

    let end_point_d = geo::algorithm::line_measures::metric_spaces::Geodesic
        .point_at_distance_between(
            point!(a_end.x_y()),
            point!(x: a_end.x()+(dy), y: a_end.y()+(-dx)),
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
            ba_line: Line::new(b_start, a_end),
        },
        LineTriangle {
            triangle: Triangle::new(start_point_d.0, end_point_c.0, end_point_d.0),
            line: *line,
            a,
            b,
            c,
            d,
            ba_line: Line::new(b_start, a_end),
        },
    )
}

pub fn probe_timestamp(start_m: f64, delta_m: f64, ratio: f64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_secs((delta_m * ratio) as i64 + start_m as i64)
        .expect("ratio er fucked")
}

pub fn vessel_speed<const CRS: u64>(
    line: &LineM<CRS>,
    distance: f64,
    line_meters: f64,
) -> TimeDelta {
    if line_meters == 0. {
        return Duration::seconds(0);
    }
    Duration::seconds(((line.end().m - line.start().m) / line_meters * distance) as i64) // how long does it take the vessel to travel 'distance', based on calculated speed
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

pub fn probe_vector(ba_line: &Line, triangle: Triangle, ba: f64, bb: f64, bc: f64) -> Coord<f64> {
    let coord = barycentric_to_cartesian(triangle, ba, bb, bc);
    coord! {x: coord.x-ba_line.start().x, y: coord.y-ba_line.start().y}
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
    fn half_way_test_with_matching_a_b() {
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

        let a = line_to_triangle_pair(&line, 10.0, 10.0, 10.0, 10.0);

        assert_eq!(
            (a.0.point_occupation(1. / 2., 0., 1. / 2.).1.timestamp() as f64 - start_m
                + a.0.point_occupation(1. / 2., 0., 1. / 2.).0.timestamp() as f64
                - start_m)
                / 2.0,
            (end_m - start_m) / 2.0
        ) // point_occupation returns a start and end time for a probe, if we are probing the middle and a = b (this test),
        // then (∆probe_start_time+∆probe_endtime)/2 should be = ∆delta_m / 2
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
        assert_eq!(a.0.point_occupation(0., 1., 0.).1.timestamp(), end_m as i64);
    }
}
