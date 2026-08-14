/// Where a long-running step says how far it has got, supplied by whoever called it.
///
/// ADR 0001 decision 4: the library reports to a sink it is given and never to a stream it chose, so
/// nothing here names a destination. The one method is the whole surface a caller has to implement,
/// which is what keeps a step's signature from asking for a reporting *facility*.
///
/// `&mut self` because a sink that counts, throttles or draws needs state of its own; `total` on
/// every call rather than at construction because the sink is the caller's value, built before it
/// knows what it will be handed.
pub trait Progress {
    /// `done` of `total` units finished, both absolute. A sink that missed a call still reports the
    /// right position, and a step that resumes or emits out of order needs no delta the caller would
    /// have to reassemble.
    fn advance(&mut self, done: u64, total: u64);
}

/// The sink for a caller that wants no reporting, so a step takes `()` instead of growing a second
/// signature without the parameter.
impl Progress for () {
    fn advance(&mut self, _done: u64, _total: u64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct Counting {
        calls: u32,
        last: Option<(u64, u64)>,
    }

    impl Progress for Counting {
        fn advance(&mut self, done: u64, total: u64) {
            self.calls += 1;
            self.last = Some((done, total));
        }
    }

    fn drive(sink: &mut impl Progress, total: u64) {
        for done in 1..=total {
            sink.advance(done, total);
        }
    }

    #[test]
    fn the_last_advance_a_sink_sees_is_the_finished_pair() {
        let mut sink = Counting::default();
        drive(&mut sink, 4);

        assert_eq!(sink.calls, 4);
        assert_eq!(sink.last, Some((4, 4)));
    }

    #[test]
    fn the_unit_sink_satisfies_the_same_bound() {
        // There is no behaviour to assert. What this pins is that `()` passes where a real sink does,
        // which is the whole reason the implementation exists.
        drive(&mut (), 4);
    }
}
