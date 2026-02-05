use crate::types::coordm::CoordM;
use crate::types::error::Error;
use crate::types::linestringm::LineStringM;
use crate::types::pointm::PointM;
use geo_traits::{PointTrait, UnimplementedMultiLineString};
use geo_traits::{
    CoordTrait, GeometryTrait, LineStringTrait, MultiLineStringTrait, MultiPointTrait,
    UnimplementedGeometryCollection, UnimplementedLine, UnimplementedMultiPoint,
    UnimplementedMultiPolygon, UnimplementedPolygon, UnimplementedRect, UnimplementedTriangle,
};

// #[derive(Debug, Clone, PartialEq, Hash)]
pub struct MultiLineStringM<const CRS: u64 = 4326>(pub Vec<LineStringM<CRS>>);

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct MultiPointM<const CRS: u64 = 4326>(pub Vec<PointM<CRS>>);

// impl<const CRS: u64> From<Vec<LineStringM<CRS>>> for MultiLineStringM<CRS> {
//     fn from(value: Vec<LineStringM<CRS>>) -> Self {
//         MultiLineStringM(value)
//     }
// }
impl<const CRS: u64> From<Vec<PointM<CRS>>> for MultiPointM<CRS> {
    fn from(value: Vec<PointM<CRS>>) -> Self {
        MultiPointM(value)
    }
}

// impl<const CRS: u64> TryFrom<wkb::reader::Wkb<'_>> for MultiLineStringM<CRS> {
//     type Error = super::error::Error;

//     fn try_from(value: wkb::reader::Wkb<'_>) -> Result<Self, Self::Error> {
//         match value.as_type() {
//             geo_traits::GeometryType::MultiLineString(mls) => {
//                 let lss = mls
//                     .line_strings()
//                     .map(|ls| {
//                         ls.coords()
//                             .map(|c| {
//                                 Some(CoordM::<CRS> {
//                                     x: c.x(),
//                                     y: c.y(),
//                                     m: c.nth(2)?,
//                                 })
//                             })
//                             .collect::<Option<Vec<_>>>()
//                             .map(LineStringM)
//                             .ok_or(Error::Dimension)
//                     })
//                     .collect::<Result<Vec<_>, super::error::Error>>()
//                     .map(MultiLineStringM)?;
//                 Ok(lss)
//             }
//             _ => Err(super::error::Error::IncompatibleType),
//         }
//     }
// }

impl<const CRS: u64> TryFrom<wkb::reader::Wkb<'_>> for MultiPointM<CRS> {
    type Error = super::error::Error;

    fn try_from(value: wkb::reader::Wkb<'_>) -> Result<Self, Self::Error> {
        match value.as_type() {
            geo_traits::GeometryType::MultiPoint(mp) => {
                let mps = mp
                    .points()
                    .map(|p| {
                        Some(PointM::<CRS> {
                            coord: CoordM {
                                x: p.coord()?.x(),
                                y: p.coord()?.y(),
                                m: p.coord()?.nth(2)?,
                            },
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                    .map(|mps| MultiPointM::from(mps))
                    .ok_or(Error::Dimension);
                mps
            }
            _ => Err(Error::IncompatibleType),
        }
    }
}

// impl<const CRS: u64> MultiLineStringTrait for MultiLineStringM<CRS> {
//     type InnerLineStringType<'a>
//         = LineStringM<CRS>
//     where
//         Self: 'a;

//     fn num_line_strings(&self) -> usize {
//         self.0.len()
//     }

//     unsafe fn line_string_unchecked(&self, i: usize) -> Self::InnerLineStringType<'_> {
//         self.0[i].clone()
//     }
// }

impl<const CRS: u64> MultiPointTrait for MultiPointM<CRS> {
    type InnerPointType<'a>
        = PointM<CRS>
    where
        Self: 'a;

    fn num_points(&self) -> usize {
        self.0.len()
    }

    unsafe fn point_unchecked(&self, i: usize) -> Self::InnerPointType<'_> {
        self.0[i].clone()
    }
}

impl<const CRS: u64> GeometryTrait for MultiPointM<CRS> {
    type T = f64;

    type PointType<'a>
        = PointM<CRS>
    where
        Self: 'a;

    type LineStringType<'a>
        = LineStringM<CRS>
    where
        Self: 'a;

    type PolygonType<'a>
        = UnimplementedPolygon<Self::T>
    where
        Self: 'a;

    type MultiPointType<'a>
        = MultiPointM<CRS>
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
        geo_traits::GeometryType::MultiPoint(self)
    }
}



// GeometryTrait for MultiLineStringM<CRS> {
//     type T = f64;

//     type PointType<'a>
//         = PointM<CRS>
//     where
//         Self: 'a;

//     type LineStringType<'a>
//         = LineStringM<CRS>
//     where
//         Self: 'a;

//     type PolygonType<'a>
//         = UnimplementedPolygon<Self::T>
//     where
//         Self: 'a;

//     type MultiPointType<'a>
//         = UnimplementedMultiPoint<Self::T>
//     where
//         Self: 'a;

//     type MultiLineStringType<'a>
//         = MultiLineStringM<CRS>
//     where
//         Self: 'a;

//     type MultiPolygonType<'a>
//         = UnimplementedMultiPolygon<Self::T>
//     where
//         Self: 'a;

//     type GeometryCollectionType<'a>
//         = UnimplementedGeometryCollection<Self::T>
//     where
//         Self: 'a;

//     type RectType<'a>
//         = UnimplementedRect<Self::T>
//     where
//         Self: 'a;

//     type TriangleType<'a>
//         = UnimplementedTriangle<Self::T>
//     where
//         Self: 'a;

//     type LineType<'a>
//         = UnimplementedLine<Self::T>
//     where
//         Self: 'a;

//     fn dim(&self) -> geo_traits::Dimensions {
//         geo_traits::Dimensions::Xym
//     }

//     fn as_type(
//         &self,
//     ) -> geo_traits::GeometryType<
//         '_,
//         Self::PointType<'_>,
//         Self::LineStringType<'_>,
//         Self::PolygonType<'_>,
//         Self::MultiPointType<'_>,
//         Self::MultiLineStringType<'_>,
//         Self::MultiPolygonType<'_>,
//         Self::GeometryCollectionType<'_>,
//         Self::RectType<'_>,
//         Self::TriangleType<'_>,
//         Self::LineType<'_>,
//     > {
//         geo_traits::GeometryType::MultiLineString(self)
//     }
// }

//TODO: linestring iterator
