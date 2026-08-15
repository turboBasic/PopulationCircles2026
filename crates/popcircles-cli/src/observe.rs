// Both stderr sinks: the progress meter a long-running step reports through, and the `log::Log` the
// binary installs. They sit together because they share the one stream and have to agree about it —
// the meter writes a line without a newline and the logger clears it before writing its own.
//
// This is where choosing a stream, a level and a format lives, which is what keeps those choices out of
// the library. `progress::Progress` and `log::Record` arrive as values; nothing below this module knows
// what happens to them.

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

use log::{LevelFilter, Metadata, Record};
use popcircles::progress::Progress;

/// Progress on stderr, which is the sink's other half: the library reports through a sink,
/// and choosing the stream is this crate's business.
///
/// One line, redrawn per whole percent, and silent when stderr is not a terminal — a redraw in a log
/// file is a hundred lines of carriage returns.
#[derive(Debug)]
pub(crate) struct StderrProgress {
    interactive: bool,
    percent: Option<u64>,
}

impl StderrProgress {
    pub(crate) fn new() -> Self {
        Self {
            interactive: std::io::stderr().is_terminal(),
            percent: None,
        }
    }

    pub(crate) fn finish(&mut self) {
        if self.interactive && self.percent.is_some() {
            eprintln!();
        }
    }
}

impl Progress for StderrProgress {
    fn advance(&mut self, done: u64, total: u64) {
        if !self.interactive || total == 0 {
            return;
        }
        let percent = done * 100 / total;
        if self.percent == Some(percent) {
            return;
        }
        self.percent = Some(percent);

        // A meter that cannot be drawn is not worth failing a build over: the document on stdout is
        // the result, and this is only how far it has got.
        let mut stderr = std::io::stderr();
        let _ = write!(stderr, "\r{percent:>3}% of {total} rows");
        let _ = stderr.flush();
    }
}

/// One record per line on stderr: the library emits through the facade and
/// this crate is the only place a diagnostic reaches a stream.
///
/// The elapsed figure is what makes a duration a subtraction over two lines, and it is milliseconds since
/// the process started rather than a wall-clock time — the weaker of the two deliberately, because a
/// monotonic elapsed figure is in `std` and a formatted timestamp is a datetime library.
#[derive(Debug)]
pub(crate) struct StderrLog {
    started: Instant,
    level: LevelFilter,
    interactive: bool,
}

/// A record rendered, split out from the writing so the format is pinned by a test rather than by reading
/// a process's stderr. It is what box 7's subtraction rests on.
pub(crate) fn line(elapsed: Duration, record: &Record<'_>) -> String {
    format!(
        "{:>6}ms {:<5} {}: {}",
        elapsed.as_millis(),
        record.level(),
        record.target(),
        record.args()
    )
}

impl StderrLog {
    /// Installs it, and leaves the process's level alone if something already has.
    ///
    /// The `Result` is handled rather than unwrapped for `StderrProgress::advance`'s reason one level up: a
    /// diagnostic that cannot be printed is not a reason to lose the document on stdout.
    pub(crate) fn install(started: Instant, level: LevelFilter) {
        let logger = Self {
            started,
            level,
            interactive: std::io::stderr().is_terminal(),
        };
        if log::set_boxed_logger(Box::new(logger)).is_ok() {
            log::set_max_level(level);
        }
    }
}

impl log::Log for StderrLog {
    /// Against the filter this value holds, not `log::max_level()`. The latter is process-global and
    /// `cargo test` runs a binary's unit tests as parallel threads in one process, so a check built on it
    /// would answer according to whichever test called `set_max_level` last.
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        // `StderrProgress::advance` leaves the cursor mid-line by design, so the meter's line is erased
        // before a record lands on top of it. The knowledge runs one way and that is the point: a logger
        // that clears a line something may have drawn learns nothing about the meter, where a meter
        // redrawn after a record would have to hold the logger.
        let clear = if self.interactive { "\r\x1b[2K" } else { "" };
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "{clear}{}", line(self.started.elapsed(), record));
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_renders_to_the_line_a_duration_is_subtracted_from() {
        let rendered = line(
            Duration::from_millis(1234),
            &Record::builder()
                .level(log::Level::Info)
                .target("popcircles::table")
                .args(format_args!("built 18 rows"))
                .build(),
        );
        assert_eq!(rendered, "  1234ms INFO  popcircles::table: built 18 rows");
    }

    #[test]
    fn a_level_filters_the_records_beneath_it() {
        use log::Log;

        let logger = StderrLog {
            started: Instant::now(),
            level: LevelFilter::Warn,
            interactive: false,
        };
        let at = |level| {
            logger.enabled(
                &Metadata::builder()
                    .level(level)
                    .target("popcircles::table")
                    .build(),
            )
        };
        assert!(!at(log::Level::Info));
        assert!(at(log::Level::Error));
    }
}
