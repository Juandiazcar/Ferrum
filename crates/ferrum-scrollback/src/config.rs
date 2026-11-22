//! Scrollback configuration (`FR-TERM-011`).

use crate::error::ScrollbackError;
use crate::limits::{DEFAULT_MAX_LINES, DEFAULT_SPILL_THRESHOLD_LINES, HOT_CHUNK_LINES, MAX_LINES};

/// How many lines a session keeps and when cold history leaves RAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbackConfig {
    max_lines: usize,
    spill_threshold_lines: usize,
}

impl Default for ScrollbackConfig {
    fn default() -> Self {
        Self { max_lines: DEFAULT_MAX_LINES, spill_threshold_lines: DEFAULT_SPILL_THRESHOLD_LINES }
    }
}

impl ScrollbackConfig {
    /// A validated configuration.
    ///
    /// `max_lines` is the most lines retained (0 disables scrollback; at most
    /// [`MAX_LINES`]). `spill_threshold_lines` is how many lines may stay in RAM before the oldest
    /// sealed chunks spill (at least [`HOT_CHUNK_LINES`]). Without a working spill store the
    /// retained history is also capped at the threshold, so RAM never exceeds it.
    ///
    /// # Errors
    /// [`ScrollbackError::Config`] when a value is out of range.
    pub fn new(max_lines: usize, spill_threshold_lines: usize) -> Result<Self, ScrollbackError> {
        if max_lines > MAX_LINES {
            return Err(ScrollbackError::Config("max_lines is above 1 000 000"));
        }
        if spill_threshold_lines < HOT_CHUNK_LINES {
            return Err(ScrollbackError::Config("spill threshold is below one hot chunk"));
        }
        Ok(Self { max_lines, spill_threshold_lines })
    }

    /// Most lines retained.
    pub const fn max_lines(&self) -> usize {
        self.max_lines
    }

    /// Lines allowed in RAM before spilling.
    pub const fn spill_threshold_lines(&self) -> usize {
        self.spill_threshold_lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_requirement_and_ranges_are_checked() {
        let config = ScrollbackConfig::default();
        assert_eq!((config.max_lines(), config.spill_threshold_lines()), (10_000, 50_000));
        assert!(ScrollbackConfig::new(0, 1_024).is_ok());
        assert!(ScrollbackConfig::new(1_000_000, 50_000).is_ok());
        assert!(ScrollbackConfig::new(1_000_001, 50_000).is_err());
        assert!(ScrollbackConfig::new(10, 1_023).is_err());
    }
}
