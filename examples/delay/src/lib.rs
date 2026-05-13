//! Feedback delay line built on `tympan-ladspa`.
//!
//! For each input sample `x[n]` the plugin reads the sample written
//! `delay_samples` ago, mixes it into the output, and stores the
//! input — plus a feedback proportion of the delayed sample —
//! back into the buffer at the current write index. The buffer wraps
//! around modulo its length.
//!
//! ```text
//! buffer[write]     ← x[n] + feedback · delayed
//! y[n]              ← (1 − mix) · x[n] + mix · delayed
//! write             ← (write + 1) mod buffer.len()
//! ```
//!
//! The internal storage is a plain `Vec<f32>` allocated once in
//! [`Plugin::instantiate`]. It is **not** the framework's
//! [`SpscRing`](tympan_ladspa::realtime::ring) primitive — that one
//! is FIFO push/pop with no random access, whereas a delay line needs
//! a circular buffer with constant-time random read at "N samples
//! ago" and always-succeeds overwrite-on-write. The two abstractions
//! solve different problems despite the similar "ring buffer" name.
//!
//! # Realtime logging
//!
//! This example also wires up the framework's
//! [`LogSink`](tympan_ladspa::realtime::log::LogSink) on a tiny custom
//! event enum. When the host pushes a `Feedback` control value that
//! exceeds the plugin's safety cap (0.95), the plugin clamps it and
//! emits a [`DelayLogEvent::FeedbackClamped`] event from inside
//! `run`. A background thread (spawned by `LogSink::new` during
//! [`Plugin::instantiate`]) consumes the queue and prints to stderr.
//! The realtime path itself stays allocation-free: `LogSink::log`
//! forwards directly to the underlying SPSC ring's `try_push`.
//!
//! # Caveats
//!
//! - Delay-time changes between `run()` calls are sampled at the start
//!   of each call; large jumps will click. Real-world delays
//!   interpolate; this example does not.
//! - Feedback is capped at 0.95 to prevent runaway gain at host-set
//!   values close to 1.0.

use tympan_ladspa::{
    plugin_entry,
    port::{PortDefault, PortDescriptor, Ports},
    raw::Data,
    realtime::{log::LogSink, RealtimeContext},
    InstantiateError, Plugin,
};

/// Maximum delay time the plugin can express, in milliseconds. The
/// buffer is sized for this duration at instantiate time and never
/// resized.
const MAX_DELAY_MS: f32 = 2_000.0;

/// Cap applied to the host's `Feedback` control value to avoid
/// runaway gain on a self-resonating delay line.
const FEEDBACK_CAP: f32 = 0.95;

/// Bounded capacity of the diagnostic log queue. 64 events absorb
/// the worst case of "feedback clamped on every `run` call for one
/// second" with plenty of headroom at typical audio buffer rates.
const LOG_CAPACITY: usize = 64;

/// Diagnostic events emitted by the plugin from the realtime path.
///
/// The realtime side enqueues these into a [`LogSink`]; the off-
/// thread drainer prints them to stderr. Plugin authors typically
/// define their own event enum like this — a small, `Copy` type so
/// pushing it onto the queue is a cheap memcpy.
#[derive(Debug, Clone, Copy)]
pub enum DelayLogEvent {
    /// The host wrote a `Feedback` value greater than [`FEEDBACK_CAP`].
    /// The plugin clamped the value before using it; the original
    /// request is included for diagnostic purposes.
    FeedbackClamped { requested: f32 },
}

/// Plugin per-instance state.
pub struct Delay {
    sample_rate: u32,
    buffer: Vec<Data>,
    /// Index where the next input sample will be written.
    write_index: usize,
    /// Realtime-safe sink for diagnostic events. The drainer thread
    /// it owns is joined when this field drops, which happens when
    /// the LADSPA host calls `cleanup` and the framework reclaims
    /// the [`Delay`] instance.
    logger: LogSink<DelayLogEvent>,
}

impl Plugin for Delay {
    /// Arbitrary value, not coordinated with the LADSPA central
    /// registry (see ADR 0004).
    const UNIQUE_ID: u32 = 34_567;
    const LABEL: &'static str = "tympan_delay";
    const NAME: &'static str = "Tympan Feedback Delay";
    const MAKER: &'static str = "tympan-ladspa";
    const COPYRIGHT: &'static str = "MIT OR Apache-2.0";

    fn ports() -> &'static [PortDescriptor] {
        static PORTS: &[PortDescriptor] = &[
            PortDescriptor::audio_input("In"),
            PortDescriptor::audio_output("Out"),
            PortDescriptor::control_input("Delay (ms)")
                .with_default(PortDefault::Hundred)
                .with_bounds(0.0, MAX_DELAY_MS),
            PortDescriptor::control_input("Feedback")
                .with_default(PortDefault::Middle)
                .with_bounds(0.0, FEEDBACK_CAP),
            PortDescriptor::control_input("Mix")
                .with_default(PortDefault::Middle)
                .with_bounds(0.0, 1.0),
        ];
        PORTS
    }

    fn instantiate(sample_rate: u32) -> Result<Self, InstantiateError> {
        // Buffer size in samples covers MAX_DELAY_MS. `+1` ensures
        // `delay_samples.min(buffer.len() - 1)` leaves at least one
        // slot of headroom even at the maximum delay.
        let max_samples = ((MAX_DELAY_MS / 1_000.0) * sample_rate as f32).ceil() as usize + 1;
        let buffer = vec![0.0; max_samples];
        // The drainer closure runs on a background thread, so any
        // formatting / I/O it does is fine. The realtime path never
        // enters this closure.
        let logger = LogSink::new(LOG_CAPACITY, |event| {
            eprintln!("[tympan-delay] {event:?}");
        });
        Ok(Self {
            sample_rate,
            buffer,
            write_index: 0,
            logger,
        })
    }

    fn activate(&mut self) {
        // Spec-blessed place to reset transient state. Clears the
        // delay buffer so a re-activated plugin starts silent.
        self.buffer.fill(0.0);
        self.write_index = 0;
    }

    fn run(&mut self, _rt: &RealtimeContext, _frames: usize, ports: &mut Ports<'_>) {
        let delay_ms = ports.control_input(2);
        let feedback_raw = ports.control_input(3);
        let feedback = feedback_raw.clamp(0.0, FEEDBACK_CAP);
        if feedback_raw > FEEDBACK_CAP {
            // Realtime-safe: a bounded SPSC `try_push`. We ignore the
            // success flag — a full queue means previous events are
            // still pending; dropping the current one is fine.
            let _ = self.logger.log(DelayLogEvent::FeedbackClamped {
                requested: feedback_raw,
            });
        }
        let mix = ports.control_input(4).clamp(0.0, 1.0);

        // Convert delay time to a whole-sample offset for this run().
        let delay_samples = (delay_ms.max(0.0) * 0.001 * self.sample_rate as f32) as usize;

        let (input, output) = ports.audio_in_out(0, 1);
        apply_delay(
            &mut self.buffer,
            &mut self.write_index,
            input,
            output,
            delay_samples,
            feedback,
            mix,
        );
    }
}

/// Per-sample delay-line processing, factored out of [`Delay::run`]
/// so it is testable with plain Rust slices.
fn apply_delay(
    buffer: &mut [Data],
    write_index: &mut usize,
    input: &[Data],
    output: &mut [Data],
    delay_samples: usize,
    feedback: Data,
    mix: Data,
) {
    let len = buffer.len();
    if len == 0 {
        return;
    }
    // Cap to one less than the buffer so the read index can always be
    // computed without aliasing the write index in the degenerate
    // delay == buffer length case.
    let delay = delay_samples.min(len - 1);

    for (in_sample, out_sample) in input.iter().zip(output.iter_mut()) {
        let read_index = (*write_index + len - delay) % len;
        let delayed = buffer[read_index];

        *out_sample = (1.0 - mix) * *in_sample + mix * delayed;
        buffer[*write_index] = *in_sample + feedback * delayed;

        *write_index = (*write_index + 1) % len;
    }
}

plugin_entry!(Delay);

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `input` through `apply_delay` starting from a zeroed
    /// buffer; return the produced output.
    fn run_once(
        input: &[Data],
        buffer_len: usize,
        delay: usize,
        feedback: Data,
        mix: Data,
    ) -> Vec<Data> {
        let mut buffer = vec![0.0; buffer_len];
        let mut write_index = 0;
        let mut output = vec![0.0; input.len()];
        apply_delay(
            &mut buffer,
            &mut write_index,
            input,
            &mut output,
            delay,
            feedback,
            mix,
        );
        output
    }

    #[test]
    fn dry_signal_is_identity() {
        // mix == 0 → output == input regardless of delay/feedback.
        let out = run_once(&[0.5, 0.25, 0.75, 1.0], 16, 4, 0.5, 0.0);
        assert_eq!(out, vec![0.5, 0.25, 0.75, 1.0]);
    }

    #[test]
    fn wet_signal_with_zero_feedback_is_pure_delayed_input() {
        // mix == 1, feedback == 0, delay == 1 → output[n] = input[n-1]
        // with zeros for the first `delay` samples (buffer starts
        // silent).
        let out = run_once(&[1.0, 2.0, 3.0, 4.0], 16, 1, 0.0, 1.0);
        assert_eq!(out, vec![0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn long_delay_outputs_silence_before_the_signal_arrives() {
        let out = run_once(&[1.0, 1.0, 1.0, 1.0], 16, 8, 0.0, 1.0);
        assert_eq!(out, vec![0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn feedback_accumulates_in_the_buffer() {
        // delay == 2, feedback == 0.5, mix == 1.
        // Input: single impulse [1, 0, 0, 0, 0, 0, 0, 0]
        // The impulse re-emerges every 2 samples, halved each time.
        //
        //   step 0: read buf[0]=0, write buf[0]=1+0.5*0=1     out=0
        //   step 1: read buf[1]=0, write buf[1]=0+0.5*0=0     out=0
        //   step 2: read buf[0]=1, write buf[2]=0+0.5*1=0.5   out=1
        //   step 3: read buf[1]=0, write buf[3]=0+0.5*0=0     out=0
        //   step 4: read buf[2]=0.5, write buf[0]=0+0.5*0.5=0.25, out=0.5
        //   step 5: read buf[3]=0, write buf[1]=0+0.5*0=0     out=0
        //   step 6: read buf[0]=0.25, write buf[2]=0+0.5*0.25 out=0.25
        //   step 7: read buf[1]=0, write buf[3]=0+0.5*0=0     out=0
        let out = run_once(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 4, 2, 0.5, 1.0);
        assert_eq!(out, vec![0.0, 0.0, 1.0, 0.0, 0.5, 0.0, 0.25, 0.0]);
    }

    #[test]
    fn half_mix_blends_dry_and_wet() {
        // delay 1, no feedback, mix 0.5.
        // Output should be 0.5 * input + 0.5 * delayed_input.
        let out = run_once(&[1.0, 0.5, 0.25, 0.125], 16, 1, 0.0, 0.5);
        assert_eq!(
            out,
            vec![
                0.5 * 1.0,                // 0.5*1 + 0.5*0
                0.5 * 0.5 + 0.5 * 1.0,    // 0.75
                0.5 * 0.25 + 0.5 * 0.5,   // 0.375
                0.5 * 0.125 + 0.5 * 0.25, // 0.1875
            ]
        );
    }

    #[test]
    fn write_index_wraps_modulo_buffer_length() {
        // Push 6 samples through a 4-slot buffer; the write index
        // must end up at 6 % 4 == 2.
        let mut buffer = vec![0.0; 4];
        let mut wi = 0;
        let mut out = vec![0.0; 6];
        apply_delay(
            &mut buffer,
            &mut wi,
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            &mut out,
            0,
            0.0,
            0.0,
        );
        assert_eq!(wi, 2);
    }

    #[test]
    fn activate_clears_state() {
        let mut delay = Delay::instantiate(48_000).unwrap();
        // Pollute the state.
        delay.buffer[0] = 0.42;
        delay.write_index = 17;
        delay.activate();
        assert!(delay.buffer.iter().all(|&s| s == 0.0));
        assert_eq!(delay.write_index, 0);
    }

    #[test]
    fn delay_longer_than_buffer_is_clamped() {
        // Asking for a delay equal to the buffer length should not
        // panic; it gets capped at len - 1 and the test merely
        // confirms apply_delay returns without going out of bounds.
        let mut buffer = vec![0.0; 4];
        let mut wi = 0;
        let mut out = vec![0.0; 4];
        apply_delay(
            &mut buffer,
            &mut wi,
            &[1.0; 4],
            &mut out,
            usize::MAX,
            0.0,
            1.0,
        );
        // Output starts silent (read positions one ahead of the
        // freshly-written samples). Behaviour-wise we only need to
        // assert the call did not panic.
    }
}
