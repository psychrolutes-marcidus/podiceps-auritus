use std::ops::RangeInclusive;

pub type ConfidenceInterval = RangeInclusive<f64>;
pub type Draught = f64;

const fn draught_deviation_from_confidence(
    draught: Draught,
    confidence: ConfidenceInterval,
) -> f64 {
    match *confidence.start() <= draught && draught <= *confidence.end() {
        true => 0_f64,
        false => {
            let left = *confidence.start() - draught; // draught is to the left of interval
            let right = draught - *confidence.end(); // to the right
            let closest = if draught < *confidence.start() {
                left
            } else {
                right
            };
            closest
        }
    }
}

#[cfg(test)]
mod test {

    use std::ops::RangeInclusive;

    use super::*;

    #[test]
    fn a() {
        let interval = 1_f64..=10_f64;
        let b = interval.contains(&5_f64);
    }
}
