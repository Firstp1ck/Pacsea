/// What: Track the last logged state to avoid repeating identical debug lines.
///
/// Inputs:
/// - `T`: A clonable, comparable state type to track between log attempts.
///
/// Output:
/// - `ChangeLogger`: Helper that indicates whether a new log should be emitted.
///
/// Details:
/// - Calling `should_log` updates the cached state and returns `true` only when
///   the provided state differs from the previous one.
/// - `clear` resets the cached state, forcing the next call to log.
pub struct ChangeLogger<T> {
    last: Option<T>,
}

impl<T: PartialEq + Clone> Default for ChangeLogger<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: PartialEq + Clone> ChangeLogger<T> {
    /// What: Create a new `ChangeLogger` with no cached state.
    ///
    /// Inputs:
    /// - None
    ///
    /// Output:
    /// - A `ChangeLogger` ready to track state changes.
    ///
    /// Details:
    /// - The first call to `should_log` after construction will return `true`.
    #[must_use]
    pub const fn new() -> Self {
        Self { last: None }
    }

    /// What: Decide whether a log should be emitted for the provided state.
    ///
    /// Inputs:
    /// - `next`: The next state to compare against the cached state.
    ///
    /// Output:
    /// - `true` when the state differs from the cached value; otherwise `false`.
    ///
    /// Details:
    /// - Updates the cached state when it changes.
    #[must_use]
    pub fn should_log(&mut self, next: &T) -> bool {
        if self.last.as_ref() == Some(next) {
            return false;
        }
        self.last = Some(next.clone());
        true
    }

    /// What: Reset the cached state so the next call logs.
    ///
    /// Inputs:
    /// - None
    ///
    /// Output:
    /// - None
    ///
    /// Details:
    /// - Useful for tests or after major UI transitions.
    pub fn clear(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::ChangeLogger;

    #[test]
    /// What: Ensure `ChangeLogger` only triggers when state changes.
    ///
    /// Inputs:
    /// - Sequence of repeated and differing states.
    ///
    /// Output:
    /// - Boolean results from `should_log` reflecting change detection.
    ///
    /// Details:
    /// - First call logs, repeat skips, new value logs, clear resets behavior.
    fn change_logger_emits_only_on_change() {
        let mut logger = ChangeLogger::new();
        assert!(logger.should_log(&"a"));
        assert!(!logger.should_log(&"a"));
        assert!(logger.should_log(&"b"));
        assert!(!logger.should_log(&"b"));
        logger.clear();
        assert!(logger.should_log(&"b"));
    }
}
