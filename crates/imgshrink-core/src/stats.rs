use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Summary {
    pub found: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub elapsed_secs: f64,
}

impl Summary {
    pub fn saved_bytes(&self) -> i64 {
        self.input_bytes as i64 - self.output_bytes as i64
    }

    pub fn saved_percent(&self) -> f64 {
        if self.input_bytes == 0 {
            return 0.0;
        }
        (self.saved_bytes() as f64 / self.input_bytes as f64) * 100.0
    }

    pub fn processed(&self) -> usize {
        self.succeeded + self.skipped + self.failed + self.cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn savings_only_count_successes() {
        let summary = Summary {
            found: 10,
            succeeded: 2,
            skipped: 3,
            failed: 1,
            input_bytes: 1_000,
            output_bytes: 600,
            ..Default::default()
        };
        assert_eq!(summary.saved_bytes(), 400);
        assert!((summary.saved_percent() - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn larger_output_is_negative_saving() {
        let summary = Summary {
            input_bytes: 100,
            output_bytes: 150,
            ..Default::default()
        };
        assert_eq!(summary.saved_bytes(), -50);
        assert!(summary.saved_percent() < 0.0);
    }
}
