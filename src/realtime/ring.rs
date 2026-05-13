//! Realtime-safe single-producer single-consumer ring buffer.
//!
//! `docs/overview.md` § In scope lists "lock-free SPSC ring buffers"
//! as a framework primitive; `docs/architecture.md` § Realtime logging
//! identifies the canonical use case: capturing diagnostic events from
//! `Plugin::run` for an off-thread drainer to consume. The primitive
//! itself is general — plugin authors can use it for any one-producer,
//! one-consumer queueing problem.
//!
//! # Realtime properties
//!
//! [`Producer::try_push`] and [`Consumer::try_pop`] perform a fixed
//! number of relaxed and acquire/release atomic operations plus a
//! `T`-sized memcpy. They never allocate, never block, never call
//! into the kernel. Both are safe to invoke from `Plugin::run`.
//!
//! Constructing a queue via [`channel`] allocates the backing buffer
//! and an `Arc`; both happen exactly once, before the realtime path
//! starts, typically inside [`Plugin::instantiate`](crate::Plugin::instantiate).
//! Dropping the last [`Producer`]/[`Consumer`] half deallocates;
//! perform that drop on a non-realtime thread.
//!
//! # Thread-safety contract
//!
//! Each half is [`Send`] (move to another thread) but not [`Sync`]
//! (concurrent `&Producer` operations from two threads would race on
//! the tail index). This is enforced via a [`PhantomData<Cell<()>>`]
//! field; the negative implication is verified by a `compile_fail`
//! doctest. There is exactly one producer end and one consumer end:
//! [`Producer`] is not [`Clone`], nor is [`Consumer`].
//!
//! # Example
//!
//! ```rust
//! use tympan_ladspa::realtime::ring;
//!
//! let (mut tx, mut rx) = ring::channel::<u32>(4);
//! assert!(tx.try_push(10).is_ok());
//! assert!(tx.try_push(20).is_ok());
//! assert_eq!(rx.try_pop(), Some(10));
//! assert_eq!(rx.try_pop(), Some(20));
//! assert_eq!(rx.try_pop(), None);
//! ```

use core::cell::{Cell, UnsafeCell};
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Shared backing storage for one queue. Held by exactly one
/// [`Producer`] and one [`Consumer`] via `Arc`.
struct Inner<T> {
    /// `capacity` slots, each independently initialised when the
    /// producer publishes a value at that index and de-initialised
    /// when the consumer reads it.
    buffer: Box<[UnsafeCell<MaybeUninit<T>>]>,

    /// Number of slots. Kept alongside `buffer.len()` for the
    /// hot-path arithmetic so the optimiser doesn't have to chase
    /// the slice length through the indirection.
    capacity: usize,

    /// Monotonic count of items consumed. Read by both ends; written
    /// only by the consumer.
    head: AtomicUsize,

    /// Monotonic count of items produced. Read by both ends; written
    /// only by the producer.
    tail: AtomicUsize,
}

// SAFETY: All concurrent access to `buffer` is mediated by the atomic
// head/tail indices. The producer writes a slot only when
// `tail.wrapping_sub(head) < capacity` (slot known empty); the
// consumer reads a slot only when `head != tail` for the version of
// `tail` it observed via Acquire (slot known initialised). The
// AcqRel pair on tail/head publishes/observes those writes correctly.
// Therefore `Inner<T>` is safe to share between threads when `T: Send`.
unsafe impl<T: Send> Sync for Inner<T> {}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        // After the last Producer/Consumer drops, only this thread has
        // access to `head` and `tail`; the consumer side may have left
        // items un-popped. Drain them so their destructors run.
        // Using `get_mut` is sound (and atomic-free) because we're
        // the unique owner.
        let mut head = *self.head.get_mut();
        let tail = *self.tail.get_mut();
        while head != tail {
            let idx = head % self.capacity;
            // SAFETY: the slot at `idx` was published by the producer
            // (head < tail in monotonic terms), so it holds an
            // initialised `T` that we now drop in place.
            unsafe {
                (*self.buffer[idx].get()).assume_init_drop();
            }
            head = head.wrapping_add(1);
        }
    }
}

/// The push end of an SPSC queue.
///
/// Move (`Send`) but do not share (`!Sync`). Holding a `&Producer<T>`
/// in two threads simultaneously would race on the queue's tail index;
/// the type system rules that out.
pub struct Producer<T> {
    inner: Arc<Inner<T>>,
    _not_sync: PhantomData<Cell<()>>,
}

/// The pop end of an SPSC queue. Same `Send`/`!Sync` rules as
/// [`Producer`].
pub struct Consumer<T> {
    inner: Arc<Inner<T>>,
    _not_sync: PhantomData<Cell<()>>,
}

// SAFETY: Both halves hold only `Arc<Inner<T>>` and `PhantomData<Cell<()>>`.
// `Arc<Inner<T>>` is `Send` because `Inner<T>: Sync` (declared above)
// and `Inner<T>` itself is owned-and-droppable across threads. The
// `Cell<()>` phantom is `Send`. Hence `Producer<T>` and `Consumer<T>`
// are `Send` whenever `T: Send`.
unsafe impl<T: Send> Send for Producer<T> {}
// SAFETY: identical reasoning.
unsafe impl<T: Send> Send for Consumer<T> {}

/// Create a paired [`Producer`] / [`Consumer`] backed by a buffer of
/// `capacity` items.
///
/// Allocates once for the buffer and once for the `Arc`. This is the
/// only place in the API that allocates; subsequent push/pop
/// operations are allocation-free.
///
/// # Panics
///
/// Panics if `capacity == 0`. A zero-capacity SPSC queue would have
/// `head == tail` permanently and `try_push` would always fail; it is
/// almost certainly a programmer error.
pub fn channel<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    assert!(capacity > 0, "channel capacity must be non-zero");
    let mut buffer = Vec::with_capacity(capacity);
    for _ in 0..capacity {
        buffer.push(UnsafeCell::new(MaybeUninit::<T>::uninit()));
    }
    let inner = Arc::new(Inner {
        buffer: buffer.into_boxed_slice(),
        capacity,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
    });
    let producer = Producer {
        inner: Arc::clone(&inner),
        _not_sync: PhantomData,
    };
    let consumer = Consumer {
        inner,
        _not_sync: PhantomData,
    };
    (producer, consumer)
}

impl<T> Producer<T> {
    /// Attempt to enqueue `value`.
    ///
    /// Returns `Ok(())` on success. Returns `Err(value)` if the queue
    /// is full; the caller can decide whether to drop the value, retry
    /// later, or surface the failure.
    ///
    /// # Realtime
    ///
    /// Two atomic loads (tail relaxed, head acquire), one `T`-sized
    /// memcpy, one atomic store (tail release). No allocation, no
    /// syscall, no waiting.
    pub fn try_push(&self, value: T) -> Result<(), T> {
        // Only this thread (the unique producer) writes tail, so a
        // relaxed read of our own previous value is sufficient.
        let tail = self.inner.tail.load(Ordering::Relaxed);
        // The consumer publishes head with Release on each pop. Acquire
        // here synchronises with that release so we can rule out a
        // false "queue full" verdict.
        let head = self.inner.head.load(Ordering::Acquire);
        if tail.wrapping_sub(head) >= self.inner.capacity {
            return Err(value);
        }
        let idx = tail % self.inner.capacity;
        // SAFETY: `idx` is in bounds (modulo capacity, capacity ==
        // buffer.len()). The slot at `idx` is between `tail` and
        // `head + capacity` in the monotonic sequence, so the consumer
        // is not currently reading it. We are the sole writer.
        unsafe {
            (*self.inner.buffer[idx].get()).write(value);
        }
        // Release-publish the new tail so the consumer's Acquire-read
        // observes the slot's initialised contents.
        self.inner
            .tail
            .store(tail.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Maximum number of items the queue can hold.
    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }
}

impl<T> Consumer<T> {
    /// Attempt to dequeue.
    ///
    /// Returns `Some(value)` on success. Returns `None` if the queue
    /// is empty.
    ///
    /// # Realtime
    ///
    /// Two atomic loads (head relaxed, tail acquire), one `T`-sized
    /// memcpy out, one atomic store (head release). No allocation, no
    /// syscall, no waiting.
    pub fn try_pop(&self) -> Option<T> {
        let head = self.inner.head.load(Ordering::Relaxed);
        let tail = self.inner.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let idx = head % self.inner.capacity;
        // SAFETY: `tail != head` and Acquire-read of `tail` synchronises
        // with the producer's Release-store. The slot at `idx` is
        // therefore initialised. We are the sole reader and we are
        // about to advance head past it.
        let value = unsafe { (*self.inner.buffer[idx].get()).assume_init_read() };
        self.inner
            .head
            .store(head.wrapping_add(1), Ordering::Release);
        Some(value)
    }

    /// Maximum number of items the queue can hold.
    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }
}

/// Compile-time checks that the producer/consumer halves are not
/// [`Sync`]. Concurrent `&Producer` (or `&Consumer`) operations from
/// two threads would race on the queue's tail (or head), so the type
/// system must rule that out.
///
/// ```compile_fail
/// fn requires_sync<T: Sync>() {}
/// requires_sync::<tympan_ladspa::realtime::ring::Producer<u32>>();
/// ```
///
/// ```compile_fail
/// fn requires_sync<T: Sync>() {}
/// requires_sync::<tympan_ladspa::realtime::ring::Consumer<u32>>();
/// ```
#[allow(dead_code)]
fn _doc_compile_fail_anchors() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc as StdArc;
    use std::thread;

    #[test]
    fn push_pop_preserves_fifo_order() {
        let (tx, rx) = channel::<u32>(4);
        for i in 0..4 {
            tx.try_push(i).unwrap();
        }
        let mut received = Vec::new();
        while let Some(v) = rx.try_pop() {
            received.push(v);
        }
        assert_eq!(received, vec![0, 1, 2, 3]);
    }

    #[test]
    fn push_fails_when_full() {
        let (tx, _rx) = channel::<u8>(3);
        assert!(tx.try_push(1).is_ok());
        assert!(tx.try_push(2).is_ok());
        assert!(tx.try_push(3).is_ok());
        assert_eq!(tx.try_push(4), Err(4));
    }

    #[test]
    fn pop_returns_none_when_empty() {
        let (_tx, rx) = channel::<u8>(3);
        assert_eq!(rx.try_pop(), None);
    }

    #[test]
    fn alternating_push_pop_handles_wrap_around() {
        // Push and pop many more items than the capacity to drive the
        // head/tail past several wraps.
        let (tx, rx) = channel::<usize>(4);
        for i in 0..1_000 {
            tx.try_push(i).unwrap();
            assert_eq!(rx.try_pop(), Some(i));
        }
        assert_eq!(rx.try_pop(), None);
    }

    #[test]
    fn capacity_reported_through_both_ends() {
        let (tx, rx) = channel::<u8>(7);
        assert_eq!(tx.capacity(), 7);
        assert_eq!(rx.capacity(), 7);
    }

    #[test]
    #[should_panic(expected = "capacity must be non-zero")]
    fn zero_capacity_panics() {
        let _ = channel::<u8>(0);
    }

    #[test]
    fn concurrent_push_pop_preserves_order_and_count() {
        const ITEMS: usize = 50_000;
        let (tx, rx) = channel::<usize>(16);
        let producer = thread::spawn(move || {
            let mut i = 0;
            while i < ITEMS {
                if tx.try_push(i).is_ok() {
                    i += 1;
                }
                // Otherwise the queue is full and we spin until the
                // consumer makes room — a simple busy-wait keeps the
                // test free of synchronisation primitives.
            }
        });
        let mut received = Vec::with_capacity(ITEMS);
        while received.len() < ITEMS {
            if let Some(v) = rx.try_pop() {
                received.push(v);
            }
        }
        producer.join().unwrap();
        assert_eq!(received.len(), ITEMS);
        // FIFO order is preserved end-to-end: the consumer sees
        // exactly the sequence the producer pushed.
        for (i, v) in received.iter().enumerate() {
            assert_eq!(*v, i, "out-of-order item at position {i}");
        }
    }

    /// Drop-counting payload used by the leak / double-free tests.
    #[derive(Debug)]
    struct DropCounter(StdArc<AtomicUsize>);
    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn drop_runs_for_un_popped_items() {
        let counter = StdArc::new(AtomicUsize::new(0));
        {
            let (tx, _rx) = channel::<DropCounter>(8);
            for _ in 0..5 {
                tx.try_push(DropCounter(StdArc::clone(&counter))).unwrap();
            }
            // Drop tx + rx → Inner drops → remaining 5 items drop.
        }
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn every_item_drops_exactly_once_across_pop_and_inner_drop() {
        let counter = StdArc::new(AtomicUsize::new(0));
        {
            let (tx, rx) = channel::<DropCounter>(8);
            for _ in 0..4 {
                tx.try_push(DropCounter(StdArc::clone(&counter))).unwrap();
            }
            // Pop 3 of 4 — those drops fire here.
            for _ in 0..3 {
                let _ = rx.try_pop().expect("queue should not be empty");
            }
            assert_eq!(
                counter.load(Ordering::SeqCst),
                3,
                "popped items must drop on pop"
            );
            // Drop tx + rx → 1 remaining item drops.
        }
        assert_eq!(
            counter.load(Ordering::SeqCst),
            4,
            "every item must drop exactly once across pop + Inner::drop"
        );
    }

    #[test]
    fn producer_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Producer<u32>>();
        assert_send::<Consumer<u32>>();
    }
}
