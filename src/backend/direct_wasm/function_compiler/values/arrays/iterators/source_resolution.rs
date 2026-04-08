use super::*;

#[path = "source_resolution/local_source.rs"]
mod local_source;
#[path = "source_resolution/source_kind.rs"]
mod source_kind;

thread_local! {
    static ACTIVE_ITERATOR_SOURCE_SHAPES: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

struct IteratorSourceGuard {
    key: String,
}

impl Drop for IteratorSourceGuard {
    fn drop(&mut self) {
        ACTIVE_ITERATOR_SOURCE_SHAPES.with(|active| {
            active.borrow_mut().remove(&self.key);
        });
    }
}
