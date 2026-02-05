use crate::types::coordm::CoordM;
use crate::types::error::Error;
use geo::algorithm::Distance;
use geo::algorithm::GeodesicMeasure;
use geo::{Euclidean, HaversineMeasure};
use geo_traits::CoordTrait;
use geo_traits::{
    GeometryTrait, PointTrait, UnimplementedGeometryCollection, UnimplementedLine,
    UnimplementedLineString, UnimplementedMultiLineString, UnimplementedMultiPoint,
    UnimplementedMultiPolygon, UnimplementedPolygon, UnimplementedRect, UnimplementedTriangle,
};
use geo_types::{Coord, Point};
use geographiclib_rs::Geodesic;

///largely similar to a [`CoordM`], but distinctions are made in libraries, so i am going to as well :)
#[derive(Debug, Clone, Copy, PartialEq, Hash)]
pub struct PointM<const CRS: u64 = 4326> {
    pub coord: CoordM<CRS>,
}

impl<const CRS: u64> PointM<CRS> {}

impl<const CRS: u64> From<(f64, f64, f64)> for PointM<CRS> {
    fn from((first, second, third): (f64, f64, f64)) -> Self {
        PointM {
            coord: CoordM {
                x: first,
                y: second,
                m: third,
            },
        }
    }
}
impl<const CRS: u64> From<CoordM<CRS>> for PointM<CRS> {
    fn from(value: CoordM<CRS>) -> Self {
        PointM { coord: value }
    }
}

// maybe this impl can be combined with its nonborrowing equivalent
impl<const CRS: u64> From<&CoordM<CRS>> for PointM<CRS> {
    fn from(value: &CoordM<CRS>) -> Self {
        PointM {
            coord: value.to_owned(),
        }
    }
}

impl<const CRS: u64> TryFrom<wkb::reader::Wkb<'_>> for PointM<CRS> {
    type Error = super::error::Error;

    fn try_from(value: wkb::reader::Wkb<'_>) -> Result<Self, Self::Error> {
        match value.as_type() {
            geo_traits::GeometryType::Point(p) => Ok(PointM {
                coord: CoordM {
                    x: p.coord().ok_or_else(|| Error::IncompatibleType)?.x(),
                    y: p.coord().ok_or_else(|| Error::IncompatibleType)?.y(),
                    m: p.coord()
                        .ok_or_else(|| Error::IncompatibleType)?
                        .nth(2)
                        .ok_or_else(|| Error::Dimension)?,
                },
            }),
            _ => Err(super::error::Error::IncompatibleType),
        }
    }
}

//wth is this
impl<const CRS: u64> GeometryTrait for PointM<CRS> {
    type T = f64;

    type PointType<'a>
        = PointM<CRS>
    where
        Self: 'a;

    type LineStringType<'a>
        = UnimplementedLineString<Self::T>
    where
        Self: 'a;

    type PolygonType<'a>
        = UnimplementedPolygon<Self::T>
    where
        Self: 'a;

    type MultiPointType<'a>
        = UnimplementedMultiPoint<Self::T>
    where
        Self: 'a;

    type MultiLineStringType<'a>
        = UnimplementedMultiLineString<Self::T>
    where
        Self: 'a;

    type MultiPolygonType<'a>
        = UnimplementedMultiPolygon<Self::T>
    where
        Self: 'a;

    type GeometryCollectionType<'a>
        = UnimplementedGeometryCollection<Self::T>
    where
        Self: 'a;

    type RectType<'a>
        = UnimplementedRect<Self::T>
    where
        Self: 'a;

    type TriangleType<'a>
        = UnimplementedTriangle<Self::T>
    where
        Self: 'a;

    type LineType<'a>
        = UnimplementedLine<Self::T>
    where
        Self: 'a;

    fn dim(&self) -> geo_traits::Dimensions {
        geo_traits::Dimensions::Xym
    }

    fn as_type(
        &self,
    ) -> geo_traits::GeometryType<
        '_,
        Self::PointType<'_>,
        Self::LineStringType<'_>,
        Self::PolygonType<'_>,
        Self::MultiPointType<'_>,
        Self::MultiLineStringType<'_>,
        Self::MultiPolygonType<'_>,
        Self::GeometryCollectionType<'_>,
        Self::RectType<'_>,
        Self::TriangleType<'_>,
        Self::LineType<'_>,
    > {
        geo_traits::GeometryType::Point(self)
        // geo_traits::GeometryType::
    }
}

impl<const CRS: u64> PointTrait for PointM<CRS> {
    type CoordType<'a>
        = CoordM<CRS>
    where
        Self: 'a;

    fn coord(&self) -> Option<Self::CoordType<'_>> {
        Some(self.coord)
    }
}
impl<const CRS: u64> From<PointM<CRS>> for Point {
    fn from(value: PointM<CRS>) -> Self {
        Point(Coord {
            x: value.coord.x(),
            y: value.coord.y(),
        })
    }
}

impl<const CRS: u64> Distance<f64, PointM<CRS>, PointM<CRS>> for GeodesicMeasure<fn() -> Geodesic> {
    fn distance(&self, origin: PointM<CRS>, destination: PointM<CRS>) -> f64 {
        debug_assert!(
            super::consts::DEGREE_CRS.contains(&CRS),
            "Given CRS: {0} uses non-degree Uom",
            CRS
        );
        self.distance(Point::from(origin), Point::from(destination))
    }
}

impl<const CRS: u64> Distance<f64, PointM<CRS>, PointM<CRS>> for HaversineMeasure {
    fn distance(&self, origin: PointM<CRS>, destination: PointM<CRS>) -> f64 {
        debug_assert!(
            super::consts::DEGREE_CRS.contains(&CRS),
            "Given CRS: {0} uses non-degree Uom",
            CRS
        );
        self.distance(Point::from(origin), Point::from(destination))
    }
}

impl<const CRS: u64> Distance<f64, PointM<CRS>, PointM<CRS>> for Euclidean {
    fn distance(&self, origin: PointM<CRS>, destination: PointM<CRS>) -> f64 {
        debug_assert!(
            super::consts::METRIC_CRS.contains(&CRS),
            "Given CRS: {0} uses non-meter Uom",
            CRS
        );
        self.distance(Point::from(origin), Point::from(destination))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::algorithm::line_measures::metric_spaces::Geodesic;
    use pretty_assertions::{assert_eq, assert_ne};
    use wkb::reader::read_wkb;
    #[test]
    fn geodesic_distance() {
        let first = PointM::<4326>::from((1.0, 2.0, 0.0));
        let second = PointM::from((1.0, 3.0, 1.0));
        let zero_dist = GeodesicMeasure::wgs84().distance(first, first);
        assert_eq!(zero_dist, 0.0);
        let dist = GeodesicMeasure::wgs84().distance(first, second);
        assert!(dist >= 11_000.);

        // the preferred way of measuring geodesic distance
        let alternative = Geodesic.distance(first, second);
        assert_eq!(dist, alternative);
    }
    #[test]
    fn wkb_correct_type_byte() {
        let p = PointM::<4326>::from((1., 2., 3.5));
        let mut w = vec![];
        let _ = wkb::writer::write_point(
            &mut w,
            &p,
            &wkb::writer::WriteOptions {
                endianness: wkb::Endianness::LittleEndian,
            },
        )
        .unwrap();

        let bytes = 2001_u32.to_le_bytes();
        // dbg!(hex::encode(2001_u32.to_le_bytes().as_slice()));
        // assert!(false);
        assert_eq!(bytes.as_slice(), &w[1..=4]);
        // assert_eq!(2001_u32,u32::from_le_bytes(w[1..=4]));
    }

    #[test]
    fn point_from_wkb() {
        let wkbstring = include_str!("../algo/resources/point.wkb.txt");

        let bytea = hex::decode(wkbstring).unwrap();

        let wkb = read_wkb(&bytea).unwrap();

        let p = PointM::<4326>::try_from(wkb).unwrap();
        // "POINT M (12.613097190856934 56.04176712036133 1704067223)"

        assert_eq!(p.coord.x,12.613097190856934);
        assert_eq!(p.coord.y,56.04176712036133);
        assert_eq!(p.coord.m,1704067223_f64);

    }
}
