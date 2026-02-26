use chrono::{DateTime, TimeDelta, Utc};
use geo::ConvexHull;
// use itertools::*;
use itertools::Itertools;
use std::num::NonZero;
use typed_builder::TypedBuilder;

use crate::types::linestringm::LineStringM;
use crate::types::pointm::PointM;

pub const MS_TO_KNOT: f64 = 1.9438400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Classification {
    Core(usize),
    Edge(usize),
    Noise,
    Unclassified,
}

impl Classification {
    fn cluster(&self) -> Option<usize> {
        match self {
            &Classification::Core(c) | &Classification::Edge(c) => Some(c),
            _ => None,
        }
    }
}

#[derive(TypedBuilder, Debug)]
pub struct DbScanConf<Dist, const CRS: u64>
where
    Dist: Fn(&PointM<CRS>, &PointM<CRS>) -> f64 + Send + Sync,
{
    /// Minimum number of 'nearby' points within `dist_thres` to a [Classification::Core] Point
    pub(crate) min_cluster_size: NonZero<usize>,
    /// Distance from a [Classification::Edge] Point to a [Classification::Core] Point
    pub(crate) dist: Dist,
    /// Maximum distance to a [Classification::Core] point
    dist_thres: f64,
    /// Maximum Speed Over Ground (SOG) for a point to be clustered
    pub(crate) speed_thres: f32,
    /// Maximum time interval before any succeeding points are left out of cluster
    pub(crate) max_time_thres: TimeDelta,
    #[builder(setter(skip),default=Vec::new())]
    classes: Vec<Classification>,
}

impl<Dist, const CRS: u64> DbScanConf<Dist, CRS>
where
    Dist: Fn(&PointM<CRS>, &PointM<CRS>) -> f64 + Send + Sync,
{
    // inpsired by existing DBSCAN implementation https://docs.rs/dbscan/latest/src/dbscan/lib.rs.html#184-205
    fn expand_custer(
        &mut self,
        queue: &mut Vec<usize>,
        points: &[(PointM<CRS>, f32)],
        cluster_idx: usize,
        dist_thres: f64,
    ) -> bool {
        use Classification::{Core, Edge, Noise, Unclassified};
        let mut new_cluster = false;
        while let Some(i) = queue.pop() {
            let neighbors: Vec<usize> =
                self.range_query_hash_sog((&points[i].0, i), points, dist_thres);

            if neighbors.len() < self.min_cluster_size.get() {
                continue;
            }
            new_cluster = true;
            self.classes[i] = Core(cluster_idx);

            // map noise to edge
            // map unclassified to noise
            // push neighbor to queue IF element was previously unclassified
            for ele in neighbors.iter().copied() {
                // map noise labels to at least edge
                if matches!(self.classes[ele], Noise) {
                    self.classes[ele] = Edge(cluster_idx);
                }
                if !matches!(self.classes[ele], Unclassified) {
                    continue;
                } else {
                    self.classes[ele] = Noise
                }

                queue.push(ele);
            }
        }

        new_cluster
    }

    // inspired heavily by https://docs.rs/dbscan/latest/dbscan/struct.Model.html#method.run (crate: dbscan)
    pub fn run<'p>(
        &mut self,
        points: &'p [(PointM<CRS>, f32)],
    ) -> Vec<(&'p PointM<CRS>, Classification)> {
        use Classification::{Noise, Unclassified};

        self.classes = vec![Unclassified; points.len()];

        let mut cluster = 0_usize;
        let mut queue = Vec::<usize>::new();

        for i in 0..points.len() {
            if !matches!(self.classes[i], Unclassified) {
                continue;
            }
            self.classes[i] = Noise;
            queue.push(i);
            if self.expand_custer(&mut queue, points, cluster, self.dist_thres) {
                cluster += 1;
            }
        }

        let res: Vec<(&'p PointM<CRS>, Classification)> = points
            .iter()
            .map(|(p, _)| p)
            .zip(std::mem::take(&mut self.classes))
            .collect();

        res
    }

    fn range_query_hash_sog<'p>(
        &self,
        (qp, idx): (&'p PointM<CRS>, usize),
        points: &'p [(PointM<CRS>, f32)],
        dist_thres: f64,
    ) -> Vec<usize> {
        // if qp is points[i], and points[n] is not a neighbor, then points[n-1] cannot be as well, same for points[m] and points[m+1] with n<i<m
        let neighbors = points // linestrings are ordered, so 'neighbors' will only be subslice of points
            .iter()
            .enumerate()
            .skip(idx - 1)
            .take_while(|(_, (fp, f_sog))| {
                (self.dist)(qp, fp) < dist_thres && self.temporal_sog_close(qp, fp, *f_sog)
            })
            .map(|(i, _)| i);

        let mut rev_neighbors = points
            .iter()
            .enumerate()
            .rev()
            .skip(points.len() - idx) //TODO +- 1?
            .take_while(|(_, (fp, f_sog))| {
                (self.dist)(qp, fp) < dist_thres && self.temporal_sog_close(qp, fp, *f_sog)
            })
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        rev_neighbors.extend(neighbors);
        rev_neighbors
    }
    #[inline(always)]
    fn temporal_sog_close(&self, qp: &PointM<CRS>, f: &PointM<CRS>, sog: f32) -> bool {
        let f_dt = DateTime::<Utc>::from_timestamp_secs(f.coord.m as i64).unwrap();
        let qp_dt = DateTime::<Utc>::from_timestamp_secs(qp.coord.m as i64).unwrap();
        let temporally_close = (f_dt - qp_dt).abs() < self.max_time_thres;

        sog < self.speed_thres && temporally_close
    }
}

pub enum StopOrLs<const CRS: u64> {
    Stop {
        polygon: geo::Polygon,
        tz_tange: (DateTime<Utc>, DateTime<Utc>),
    },
    LS(LineStringM<CRS>),
}
pub struct Trajectory<const CRS: u64>(pub Vec<StopOrLs<CRS>>);

pub fn cluster_to_traj_with_stop_object<const CRS: u64>(
    classes: Vec<(&PointM<CRS>, Classification)>,
) -> Trajectory<CRS> {
    // use Classification::{Core, Edge, Noise, Unclassified};
    use Classification as C;

    //? order by cluster index?

    Trajectory(
        classes
            .chunk_by(|(_, a), (_, b)| match a {
                C::Core(c) | C::Edge(c) => matches!(b, C::Core(cc) | C::Edge(cc) if c == cc),
                C::Noise | C::Unclassified => matches!(b, C::Noise | C::Unclassified),
                // _ => true, //FIXME: inverse of previous match arm
            })
            .map(|c| {
                if matches!(c.first(), Some((_, C::Core(_))) | Some((_, C::Edge(_)))) {
                    let time_start = DateTime::from_timestamp_secs(
                        c.iter()
                            .map(|(p, _c)| p)
                            .min_by(|a, b| a.coord.m.total_cmp(&b.coord.m))
                            .expect("classes should be nonempty")
                            .coord
                            .m as i64,
                    )
                    .expect("timestamp should be well within bounds");
                    let time_end = DateTime::from_timestamp_secs(
                        c.iter()
                            .map(|(p, _c)| p)
                            .max_by(|a, b| a.coord.m.total_cmp(&b.coord.m))
                            .expect("classes should be nonempty")
                            .coord
                            .m as i64,
                    )
                    .expect("timestamp should be well within bounds");

                    let a = geo::LineString::from_iter(
                        c.iter()
                            .map(|(p, _c)| geo::Point::new(p.coord.x, p.coord.y)),
                    )
                    .convex_hull();

                    StopOrLs::Stop {
                        polygon: a,
                        tz_tange: (time_start, time_end),
                    }
                } else {
                    StopOrLs::LS(
                        LineStringM::<CRS>::new(c.iter().map(|(p, _c)| p.coord).collect_vec())
                            .unwrap_or_else(|| LineStringM(vec![])),
                    )
                }
            })
            .collect_vec(),
    )
}

pub fn triangulate_stop_object(
    polygon: &geo::Polygon,
) -> Result<Vec<geo::Triangle>, geo::triangulate_delaunay::TriangulationError> {
    geo::algorithm::TriangulateDelaunay::constrained_triangulation(polygon, Default::default())
}

#[cfg(test)]
pub mod test {
    use std::fs::File;

    use chrono::TimeDelta;
    use geo::{Distance, Euclidean, Geodesic};
    use geo_traits::LineStringTrait;
    use itertools::Itertools;
    use wkb::reader::read_wkb;

    use super::Classification::*;
    use crate::algo::stop_cluster::{DbScanConf, StopOrLs, cluster_to_traj_with_stop_object};
    use crate::types::linestringm::LineStringM;
    use crate::types::pointm::PointM;

    #[test]
    fn build_conf() {
        let conf = DbScanConf::builder()
            .dist(|a: &PointM<3857>, b| 0.0)
            .max_time_thres(TimeDelta::zero())
            .speed_thres(1.5)
            .min_cluster_size(42.try_into().unwrap())
            .dist_thres(1.0)
            .build();

        assert_eq!(conf.min_cluster_size, 42.try_into().unwrap());
        assert_eq!(conf.max_time_thres, TimeDelta::zero());
        assert_eq!(conf.speed_thres, 1.5);
    }

    #[test]
    fn simple_cluster_fr_fr() {
        let mut conf = DbScanConf::builder()
            // .dist(|a, b| Geodesic.distance(*a, *b))
            .dist(|a, b| ((b.coord.x - b.coord.x).powi(2) + (b.coord.y - a.coord.y).powi(2)).sqrt())
            .max_time_thres(TimeDelta::new(10, 0).unwrap())
            .min_cluster_size(3.try_into().unwrap())
            .speed_thres(20.0)
            .dist_thres(1.1)
            .build();

        let inputs = [
            (1.5, 2.2),
            (1.0, 1.1),
            (1.2, 1.4),
            (0.8, 1.0),
            (3.7, 4.0),
            (3.9, 3.9),
            (3.6, 4.1),
            (10.0, 100.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (f, s))| (PointM::<4326>::from((f, s, i as f64 * 1.0)), 1_f32))
        .collect::<Vec<_>>();

        let clusters = conf.run(&inputs);
        dbg!(&clusters);
        assert_eq!(1, clusters.iter().filter(|å| matches!(å.1, Noise)).count());

        let c = clusters.into_iter().map(|(_, c)| c).collect_vec();

        assert_eq!(
            c,
            vec![
                Edge(0),
                Core(0),
                Core(0),
                Core(0),
                Core(1),
                Core(1),
                Core(1),
                Noise
            ]
        );
    }

    #[test]
    fn cluster_to_trajectory() {
        let mut conf = DbScanConf::builder()
            // .dist(|a, b| Geodesic.distance(*a, *b))
            .dist(|a, b| ((b.coord.x - b.coord.x).powi(2) + (b.coord.y - a.coord.y).powi(2)).sqrt())
            .max_time_thres(TimeDelta::new(10, 0).unwrap())
            .min_cluster_size(3.try_into().unwrap())
            .speed_thres(20.0)
            .dist_thres(1.1)
            .build();

        let inputs = [
            (1.5, 2.2),
            (1.0, 1.1),
            (1.2, 1.4),
            (0.8, 1.0),
            (3.7, 4.0),
            (3.9, 3.9),
            (3.6, 4.1),
            (10.0, 100.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (f, s))| (PointM::<4326>::from((f, s, i as f64 * 1.0)), 1_f32))
        .collect::<Vec<_>>();

        let clusters = conf.run(&inputs);
        dbg!(&clusters);
        assert_eq!(1, clusters.iter().filter(|å| matches!(å.1, Noise)).count());

        let mut traj = cluster_to_traj_with_stop_object(clusters).0.into_iter();

        assert!(matches!(
            traj.next(),
            Some(StopOrLs::Stop {
                polygon: _,
                tz_tange: _
            })
        ));
        assert!(matches!(
            traj.next(),
            Some(StopOrLs::Stop {
                polygon: _,
                tz_tange: _
            })
        ));
        assert!(matches!(traj.next(), Some(StopOrLs::LS(_))));
        assert!(traj.next().is_none());
    }

    #[test]
    fn cluster_big_traj() {
        let mut conf = DbScanConf::builder()
            // .dist(|a, b| Geodesic.distance(*a, *b))
            .dist(|a: &PointM<4326>, b| Geodesic.distance(*a, *b))
            .max_time_thres(TimeDelta::new(30 * 60, 0).unwrap())
            .min_cluster_size(10.try_into().unwrap())
            .speed_thres(1.5)
            .dist_thres(250.0)
            .build();

        let a = include_str!("./resources/219013708.txt");
        let a = a.replace("\"", "");
        let hex = hex::decode(a).unwrap();

        let wkb = read_wkb(&hex).unwrap();

        let ls = LineStringM::<4326>::try_from(wkb).unwrap();

        let clusters = conf.run(
            &ls.points()
                .zip(std::iter::repeat(1.0_f32))
                .collect::<Vec<_>>(),
        );
    }
    #[test]
    fn cluster_big_traj_aarhus_odden() {
        let mut conf = DbScanConf::builder()
            // .dist(|a, b| Geodesic.distance(*a, *b))
            .dist(|a: &PointM<4326>, b| Geodesic.distance(*a, *b))
            .max_time_thres(TimeDelta::new(30 * 60, 0).unwrap())
            .min_cluster_size(100.try_into().unwrap())
            .speed_thres(1.5)
            .dist_thres(50.0)
            .build();

        let a = include_str!("./resources/219705000_aarhus_odden.txt");
        let a = a.replace("\"", "");
        let hex = hex::decode(a).unwrap();

        let wkb = read_wkb(&hex).unwrap();

        let ls = LineStringM::<4326>::try_from(wkb).unwrap();

        let point_with_synthetic_sog = ls
            .points()
            .zip(std::iter::repeat(1.0_f32))
            .collect::<Vec<_>>(); // all SOGS are below speed threshold.
        let clusters = conf.run(&point_with_synthetic_sog);

        let stops = cluster_to_traj_with_stop_object(clusters);

        let stops = stops
            .0
            .into_iter()
            .filter_map(|p| match p {
                StopOrLs::Stop {
                    polygon,
                    tz_tange: _,
                } => Some(polygon),
                _ => None,
            })
            .collect_vec();

        let mp = geo::MultiPolygon::new(stops.clone());
        let opt = wkb::writer::WriteOptions {
            endianness: wkb::Endianness::LittleEndian,
        };

        let mut w = Vec::<u8>::new();
        let _ = wkb::writer::write_multi_polygon(&mut w, &mp, &opt).unwrap();
        let hex = hex::encode(w);
        std::fs::write("aarhus_odden_stops.txt", hex).unwrap();
        dbg!(
            mp.0.iter()
                .min_by_key(|x| x.exterior().num_coords())
                .unwrap()
                .exterior()
                .num_coords()
        );
        assert_eq!(stops.len(), 0)
    }
}
