use tilerizer::{Point as GPoint, PointWTime, PointWZ};

/// Represents the coordinates of an MVT tile
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Cell {
    pub coord: GPoint,
    pub z: u32,
}

impl From<PointWZ> for Cell {
    fn from(value: PointWZ) -> Self {
        Cell {
            coord: GPoint {
                x: value.point.x,
                y: value.point.y,
            },
            z: value.z as u32,
        }
    }
}

impl From<PointWTime> for Cell {
    fn from(value: PointWTime) -> Self {
        Cell {
            coord: GPoint {
                x: value.point.x,
                y: value.point.y,
            },
            z: value.z as u32,
        }
    }
}

impl From<(i32, i32, u32)> for Cell {
    fn from(value: (i32, i32, u32)) -> Self {
        Cell {
            coord: GPoint {
                x: value.0,
                y: value.1,
            },
            z: value.2,
        }
    }
}
