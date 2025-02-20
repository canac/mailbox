use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Eq, PartialEq)]
pub struct Id(u64);

// A MonotonicCounter is a thread-safe counter that will generate opaque ids. It is designed to
// help ensure that when multiple asynchronous queries are made to a data source, only the freshest
// is data is used. To use, create a MonotonicCounter, and generate and save an opaque request id
// via next() when the load request is started. Then, when the load request asynchronously
// finishes, compare the request id to the last request id via last(). If they are equal, then the
// data is the freshest possible. Otherwise, the data is staler than another request in progress.
#[derive(Clone)]
pub struct MonotonicCounter {
    last_id: Arc<AtomicU64>,
}

impl MonotonicCounter {
    // Create a new monotonic counter
    pub fn new() -> Self {
        Self {
            last_id: Arc::new(AtomicU64::new(0)),
        }
    }

    // Generate a new, opaque id
    pub fn next(&self) -> Id {
        Id(self.last_id.fetch_add(1, Ordering::SeqCst) + 1)
    }

    // Re-generate the last opaque id without advancing the counter
    pub fn last(&self) -> Id {
        Id(self.last_id.load(Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_unique() {
        let counter = MonotonicCounter::new();
        assert!(counter.next() != counter.next());
    }

    #[test]
    fn test_last() {
        let counter = MonotonicCounter::new();
        assert!(counter.next() == counter.last());

        let id = counter.next();
        counter.next();
        assert!(id != counter.last());
    }
}
