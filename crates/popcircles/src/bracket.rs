//! The `debug` pair box 7 of issue #8 asks for: two records carrying one operation name, so a duration is
//! a subtraction over their elapsed prefixes rather than a stopwatch run by hand.
//!
//! Its own module rather than a corner of [`crate::progress`], because the two must not touch:
//! progress and diagnostics are kept apart, and the CLI opens pairs of its own with the same shape.

/// A `debug` pair, opened where the expensive step begins and closed by [`Drop`].
///
/// **The end record is `Drop`'s and never a call.** Every region box 7 asks to bracket is threaded with
/// `?`, so a hand-written end line is a line at each of those exits and a line the next `?` added silently
/// skips — which reports a duration that is missing precisely when someone is reading the log to find out
/// why something failed.
///
/// `target` is the **caller's**, passed in as `module_path!()` from the call site. That macro expands where
/// it is written, so a guard calling it would stamp every end record with this module instead of the one
/// the work happened in.
#[derive(Debug)]
pub struct Bracket {
    target: &'static str,
    operation: String,
    figure: Option<(&'static str, u64)>,
}

impl Bracket {
    /// Opens the pair, emitting the begin record at once.
    ///
    /// `operation` is owned because the name identifies *which* level or *which* radius as well as what
    /// kind of step it is, and a run has of the order of a hundred and seventy of them — the allocation is
    /// beneath measurement and a `&'static str` would rule the number out.
    pub fn open(target: &'static str, operation: impl Into<String>) -> Self {
        let operation = operation.into();
        log::log!(target: target, log::Level::Debug, "{operation} begins");
        Self {
            target,
            operation,
            figure: None,
        }
    }

    /// A figure the end record carries, set any time before the scope ends.
    ///
    /// What a caller wants counted is usually known only once the step is over — the kernels a search level
    /// built are a delta across it — so it cannot be an argument to [`Self::open`].
    pub fn figure(&mut self, name: &'static str, value: u64) {
        self.figure = Some((name, value));
    }
}

impl Drop for Bracket {
    fn drop(&mut self) {
        match self.figure {
            Some((name, value)) => log::log!(
                target: self.target,
                log::Level::Debug,
                "{} ends, {name} {value}",
                self.operation
            ),
            None => log::log!(target: self.target, log::Level::Debug, "{} ends", self.operation),
        }
    }
}

// expect is warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows it in tests. expect_used alone, because the tests below take a
// lock and read a captured record and never unwrap.
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    static CAPTURED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

    #[derive(Debug)]
    struct Capture;

    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &log::Record<'_>) {
            if let Ok(mut captured) = CAPTURED.lock() {
                captured.push((record.target().to_string(), record.args().to_string()));
            }
        }

        fn flush(&self) {}
    }

    /// Once for the whole process, because a logger is global and `cargo test` runs these as threads in
    /// one. The install's `Result` is discarded for that reason too: what matters is that one is there.
    fn capturing() {
        static ONCE: OnceLock<()> = OnceLock::new();
        ONCE.get_or_init(|| {
            drop(log::set_boxed_logger(Box::new(Capture)));
            log::set_max_level(log::LevelFilter::Debug);
        });
    }

    #[test]
    fn both_records_name_the_calling_module_and_the_end_one_carries_the_figure() {
        capturing();
        {
            let mut bracket = Bracket::open(module_path!(), "a load");
            bracket.figure("cells", 42);
        }

        let captured = CAPTURED.lock().expect("the capture is not poisoned");
        let pair: Vec<&(String, String)> = captured
            .iter()
            .filter(|(_, message)| message.starts_with("a load"))
            .collect();

        assert_eq!(pair.len(), 2, "{captured:?}");
        // `::tests`, not `popcircles::bracket`, which is the whole property: a `module_path!()` written
        // inside `Bracket` would put the guard's own module on both of these.
        assert_eq!(pair[0].0, "popcircles::bracket::tests");
        assert_eq!(pair[1].0, "popcircles::bracket::tests");
        assert_eq!(pair[0].1, "a load begins");
        assert_eq!(pair[1].1, "a load ends, cells 42");
    }
}
