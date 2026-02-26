use std::time::Duration;
type EuclidianDist = f32;
pub struct SeperateConf {
    pub distance: EuclidianDist, // meter
    pub time: Duration,
}

pub struct SeperateConfBuilder {
    distance: Option<EuclidianDist>,
    time: Option<Duration>,
}

impl Default for SeperateConfBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SeperateConfBuilder {
    pub fn new() -> SeperateConfBuilder {
        SeperateConfBuilder {
            distance: None,
            time: None,
        }
    }

    pub fn distance(&mut self, distance: EuclidianDist) {
        self.distance = Some(distance)
    }

    pub fn time(&mut self, time: Duration) {
        self.time = Some(time)
    }

    pub fn build(&self) -> SeperateConf {
        SeperateConf {
            distance: self.distance.unwrap_or(1000.0),
            time: self.time.unwrap_or(Duration::from_secs(60)), // Should be set to whatever we find to be the best value.
        }
    }
}

