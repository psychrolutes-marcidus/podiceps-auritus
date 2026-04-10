use std::ops::RangeInclusive;

pub type ConfidenceInterval = RangeInclusive<f64>;
pub type Draught = f64;

pub fn score_deviation(draught: Draught, confidence: &ConfidenceInterval) -> f64 {
    square_error(draught_deviation_from_confidence(draught, confidence))
}

const fn draught_deviation_from_confidence(
    draught: Draught,
    confidence: &ConfidenceInterval,
) -> f64 {
    match *confidence.start() <= draught && draught <= *confidence.end() {
        true => 0_f64,
        false => {
            let left = *confidence.start() - draught; // draught is to the left of interval
            let right = draught - *confidence.end(); // to the right
            if draught < *confidence.start() {
                left
            } else {
                right
            }
        }
    }
}

fn square_error(score: f64) -> f64 {
    score.powi(2)
}

#[cfg(test)]
mod test {

    use std::ops::RangeInclusive;

    use super::*;
    const INTERVAL: ConfidenceInterval = 1_f64..=10_f64;
    const DRAUGHT_WITHIN: f64 = 5_f64;

    #[test]
    #[ignore = "const shenanigans"]
    fn a() {
        const _: () = assert!(
            draught_deviation_from_confidence(DRAUGHT_WITHIN, &INTERVAL) == 0_f64,
            "deviation should be 0 when measurement is contained within interval"
        );
        const _: () = assert!(
            (draught_deviation_from_confidence(*INTERVAL.start(), &INTERVAL)) == 0_f64,
            "deviation should be 0 when measurement is equal to lower bound"
        );
        const _: () = assert!(
            (draught_deviation_from_confidence(*INTERVAL.end(), &INTERVAL)) == 0_f64,
            "deviation should be 0 when measurement is equal to upper bound, since it is inclusive"
        );
        const _: () = assert!(
            draught_deviation_from_confidence(*INTERVAL.start() - 1_f64, &INTERVAL) == 1_f64,
            "deviation should be 1 when it has distance of 1 to interval"
        );
    }

    #[test]
    fn square_error_is_squaring() {
        assert_eq!(score_deviation(12_f64, &INTERVAL), 4_f64);
    }
}
