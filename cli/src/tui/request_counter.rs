use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Eq, PartialEq)]
pub struct RequestId(u64);

// A RequestCounter is designed to help ensure that when multiple asynchronous queries are made to
// a data source, only the freshest is data is used. To use, create a RequestCounter, and generate
// an opaque request id via next() when the load request is started. Then, when the load request
// asynchronously finishes, compare the request id to the latest request id via is_latest(). If the
// return value is true, the data is the freshest possible. If the return value is false, the data
// is staler than another request in progress.
#[derive(Clone)]
pub struct RequestCounter {
    next_id: Arc<AtomicU64>,
}

impl RequestCounter {
    // Create a new request counter
    pub fn new() -> Self {
        RequestCounter {
            next_id: Arc::new(AtomicU64::new(0)),
        }
    }

    // Generate a new, opaque request id
    pub fn next(&self) -> RequestId {
        RequestId(self.next_id.fetch_add(1, Ordering::SeqCst))
    }

    // Check whether the opaque request id is the latest or whether another request holding fresher
    // data is already in progress
    pub fn is_latest(&self, req_id: &RequestId) -> bool {
        self.next_id.load(Ordering::SeqCst) == req_id.0 + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_unique() {
        let req_counter = RequestCounter::new();
        assert!(req_counter.next() != req_counter.next());
    }

    #[test]
    fn test_is_latest() {
        let req_counter = RequestCounter::new();
        let req_1 = req_counter.next();
        let req_2 = req_counter.next();
        assert_eq!(req_counter.is_latest(&req_1), false);
        assert_eq!(req_counter.is_latest(&req_2), true);
    }
}
