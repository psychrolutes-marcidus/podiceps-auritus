use crate::{Point, PointWTime, PointWZ};
use crate::{Zoom, point_to_grid};
use geo::TriangulateDelaunay;
use geo_types::Polygon;
use modeling::modeling::LineTriangle;
use std::cmp;

const EPS: f64 = 0.001;
const EPS_SQUARE: f64 = EPS * EPS;

pub struct Triangle {
    pub v1: Point,
    pub v2: Point,
    pub v3: Point,
}

impl Triangle {
    pub fn get_bbox(&self) -> (i32, i32, i32, i32) {
        let bbminx = cmp::min(cmp::min(self.v1.x, self.v2.x), self.v3.x);
        let bbminy = cmp::min(cmp::min(self.v1.y, self.v2.y), self.v3.y);
        let bbmaxx = cmp::max(cmp::max(self.v1.x, self.v2.x), self.v3.x);
        let bbmaxy = cmp::max(cmp::max(self.v1.y, self.v2.y), self.v3.y);

        (bbminx, bbminy, bbmaxx, bbmaxy)
    }
}

pub fn render_stop_object(
    poly: &Polygon,
    zoom_level: i32,
    sampling_zoom_level: i32,
    filter_tile: Option<(i32, i32, i32)>,
) -> Option<Vec<(i32, i32, i32)>> {
    let triangles = poly.constrained_triangulation(Default::default()).ok();
    let points: Option<Vec<_>> = triangles.map(|ts| {
        ts.iter()
            .flat_map(|t| draw_triangle(*t, sampling_zoom_level))
            .filter(|p| match filter_tile {
                Some(ft) => {
                    let point = p.change_zoom(ft.2);
                    point.point.x == ft.0 && point.point.y == ft.1
                }
                None => true,
            })
            .map(|p| p.change_zoom(zoom_level))
            .map(|p| (p.point.x, p.point.y, p.z))
            .collect()
    });

    points.map(|x| {
        let mut x = x;
        x.sort_by_cached_key(|x| *x);
        let points: Vec<_> = x
            .chunk_by(|a, b| a == b)
            .flat_map(|x| x.first().map(|x| x.to_owned()))
            .collect();
        points
    })
}

pub fn draw_line_triangle(triangle: LineTriangle<4326>, sample_zoom_level: i32) -> Vec<PointWTime> {
    let triangle_grid = real_to_grid(&triangle.triangle, sample_zoom_level);
    let (bbminx, bbminy, bbmaxx, bbmaxy) = triangle_grid.get_bbox();
    let Triangle { v1, v2, v3 } = triangle_grid;
    let size = (bbmaxx - bbminx) * (bbmaxy - bbminy);

    let mut points: Vec<PointWTime> = Vec::with_capacity(size as usize / 2 + 1);

    let total_area = signed_total_area(v1.x, v1.y, v2.x, v2.y, v3.x, v3.y);

    for x in bbminx..=bbmaxx {
        for y in bbminy..=bbmaxy {
            if let Some((alpha, beta, gamma)) =
                check_point(v1.x, v1.y, v2.x, v2.y, v3.x, v3.y, x, y, total_area)
            {
                let timestamp = triangle.point_occupation(alpha, beta, gamma);
                let point = Point { x, y };
                points.push(PointWTime {
                    point,
                    z: sample_zoom_level,
                    time_start: timestamp.0,
                    time_end: timestamp.1,
                });
            }
        }
    }
    points
}

pub fn draw_triangle(triangle: geo_types::Triangle, sample_zoom_level: i32) -> Vec<PointWZ> {
    let triangle_grid = real_to_grid(&triangle, sample_zoom_level);
    let (bbminx, bbminy, bbmaxx, bbmaxy) = triangle_grid.get_bbox();
    let Triangle { v1, v2, v3 } = triangle_grid;
    let size = (bbmaxx - bbminx) * (bbmaxy - bbminy);

    let mut points: Vec<PointWZ> = Vec::with_capacity(size as usize / 2 + 1);

    let total_area = signed_total_area(v1.x, v1.y, v2.x, v2.y, v3.x, v3.y);

    for x in bbminx..=bbmaxx {
        for y in bbminy..=bbmaxy {
            if let Some((_, _, _)) =
                check_point(v1.x, v1.y, v2.x, v2.y, v3.x, v3.y, x, y, total_area)
            {
                let point = Point { x, y };
                points.push(PointWZ {
                    point,
                    z: sample_zoom_level,
                })
            }
        }
    }
    points
}

#[allow(clippy::too_many_arguments)]
fn naive_point_in_triangle(
    v1x: i32,
    v1y: i32,
    v2x: i32,
    v2y: i32,
    v3x: i32,
    v3y: i32,
    x: i32,
    y: i32,
    total_area: f64,
) -> (f64, f64, f64) {
    let alpha = signed_total_area(x, y, v2x, v2y, v3x, v3y) / total_area;
    let beta = signed_total_area(x, y, v3x, v3y, v1x, v1y) / total_area;
    let gamma = signed_total_area(x, y, v1x, v1y, v2x, v2y) / total_area;
    (alpha, beta, gamma)
}

fn distance_square_point_to_segment(x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) -> f64 {
    let p1_p2_square_length = (x2 - x1).powi(2) + (y2 - y1).powi(2);
    let dot_product = ((x - x1) * (x2 - x1) + (y - y1) * (y2 - y1)) / p1_p2_square_length;
    if dot_product < 0.0 {
        (x - x1).powi(2) + (y - y1).powi(2)
    } else if dot_product <= 1.0 {
        let p_p1_square_length = (x1 - x).powi(2) + (y1 - y).powi(2);
        p_p1_square_length - dot_product.powi(2) * p1_p2_square_length
    } else {
        (x - x2).powi(2) + (y - y2).powi(2)
    }
}

#[allow(clippy::too_many_arguments)]
fn check_point(
    v1x: i32,
    v1y: i32,
    v2x: i32,
    v2y: i32,
    v3x: i32,
    v3y: i32,
    x: i32,
    y: i32,
    total_area: f64,
) -> Option<(f64, f64, f64)> {
    let (alpha, beta, gamma) =
        naive_point_in_triangle(v1x, v1y, v2x, v2y, v3x, v3y, x, y, total_area);
    if alpha >= 0. && beta >= 0. && gamma >= 0. {
        return Some((alpha, beta, gamma));
    }
    let x1 = v1x as f64;
    let y1 = v1y as f64;
    let x2 = v2x as f64;
    let y2 = v2y as f64;
    let x3 = v3x as f64;
    let y3 = v3y as f64;
    let x = x as f64;
    let y = y as f64;

    if distance_square_point_to_segment(x1, y1, x2, y2, x, y) <= EPS_SQUARE {
        return Some((alpha.clamp(0., 1.), beta.clamp(0., 1.), gamma.clamp(0., 1.)));
    }
    if distance_square_point_to_segment(x2, y2, x3, y3, x, y) <= EPS_SQUARE {
        return Some((alpha.clamp(0., 1.), beta.clamp(0., 1.), gamma.clamp(0., 1.)));
    }
    if distance_square_point_to_segment(x3, y3, x1, y1, x, y) <= EPS_SQUARE {
        return Some((alpha.clamp(0., 1.), beta.clamp(0., 1.), gamma.clamp(0., 1.)));
    }

    None
}

fn real_to_grid(triangle: &geo_types::Triangle, sampling_zoom_level: i32) -> Triangle {
    Triangle {
        v1: point_to_grid(triangle.0, sampling_zoom_level),
        v2: point_to_grid(triangle.1, sampling_zoom_level),
        v3: point_to_grid(triangle.2, sampling_zoom_level),
    }
}

pub fn signed_total_area(v1x: i32, v1y: i32, v2x: i32, v2y: i32, v3x: i32, v3y: i32) -> f64 {
    0.5 * ((v2y - v1y) as i64 * (v2x + v1x) as i64
        + (v3y - v2y) as i64 * (v3x + v2x) as i64
        + (v1y - v3y) as i64 * (v1x + v3x) as i64) as f64
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use linesonmaps::types::{linem::LineM, pointm::PointM};
    use modeling::modeling::line_to_triangle_pair;

    use super::*;

    #[test]
    fn draw_triangle_test() {
        // 57.01534956,9.99105250
        // 57.01322067,9.99096883

        let start_m =
            DateTime::parse_from_str("2024-01-01 00:00:00 +0000", "%Y-%m-%d %H:%M:%S%.3f %z")
                .unwrap()
                .timestamp() as f64;
        let end_m =
            DateTime::parse_from_str("2024-01-01 00:02:00 +0000", "%Y-%m-%d %H:%M:%S%.3f %z")
                .unwrap()
                .timestamp() as f64;

        let coord_1: PointM = (9.99105250, 57.01534956, start_m).into();
        let coord_2: PointM = (9.99096883, 57.01322067, end_m).into();
        let line = LineM::<4326>::from((coord_1, coord_2));
        let (a, b) = line_to_triangle_pair(&line, 50.0, 50.0, 50.0, 50.0);
        let result = draw_line_triangle(a, 20);
        let result_b = draw_line_triangle(b, 20);

        assert_eq!(result.len(), 35);
        assert_eq!(result_b.len(), 40);
    }
}
