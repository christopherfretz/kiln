//! Summary statistics over repeated runs (`spec/bench.md` "Measurement").

/// The summary of one metric over N runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    /// Median after dropping one minimum and one maximum (when N >= 3).
    pub median: f64,
    pub min: f64,
    pub max: f64,
}

impl Summary {
    /// `None` for an empty sample set.
    pub fn of(samples: &[f64]) -> Option<Self> {
        let mut s: Vec<f64> = samples.iter().copied().filter(|x| x.is_finite()).collect();
        if s.is_empty() {
            return None;
        }
        s.sort_by(f64::total_cmp);
        let (min, max) = (s[0], s[s.len() - 1]);
        let kept = if s.len() >= 3 { &s[1..s.len() - 1] } else { &s[..] };
        let mid = kept.len() / 2;
        let median =
            if kept.len() % 2 == 1 { kept[mid] } else { f64::midpoint(kept[mid - 1], kept[mid]) };
        Some(Self { median, min, max })
    }

    /// How far the median sits above the fastest run, in percent of the
    /// fastest run. Large values mean a noisy host or too few runs.
    pub fn spread_pct(&self) -> f64 {
        if self.min > 0.0 { (self.median - self.min) / self.min * 100.0 } else { 0.0 }
    }
}

/// Rounds to `digits` decimal places (records keep µs for milliseconds).
pub fn round_to(x: f64, digits: i32) -> f64 {
    let f = 10f64.powi(digits);
    (x * f).round() / f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_drops_min_and_max() {
        let s = Summary::of(&[100.0, 10.0, 12.0, 11.0, 13.0]).unwrap();
        assert_eq!((s.median, s.min, s.max), (12.0, 10.0, 100.0));
        // Even count after trimming: mean of the middle two.
        let s = Summary::of(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        assert_eq!(s.median, 3.5);
        // Fewer than 3 runs: nothing is dropped.
        assert_eq!(Summary::of(&[4.0, 2.0]).unwrap().median, 3.0);
        assert_eq!(Summary::of(&[7.0]).unwrap().median, 7.0);
        assert_eq!(Summary::of(&[]), None);
    }

    #[test]
    fn spread() {
        let s = Summary::of(&[100.0, 110.0, 120.0]).unwrap();
        assert!((s.spread_pct() - 10.0).abs() < 1e-9);
        assert_eq!(Summary { median: 0.0, min: 0.0, max: 0.0 }.spread_pct(), 0.0);
        assert_eq!(round_to(712.345_678, 3), 712.346);
    }
}
