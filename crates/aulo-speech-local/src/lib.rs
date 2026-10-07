//! What aulo's local speech engines share, so each engine crate holds only its
//! model binding: the thread that owns a loaded model ([`Worker`]), the STT
//! engine for models that decode a whole utterance ([`OfflineStt`]) and the
//! catalog file check ([`check_installed`]). It links no model runtime.

mod model;
mod offline;
mod worker;

pub use model::check_installed;
pub use offline::{
    MAX_TRANSCRIPT_BYTES, MAX_UTTERANCE_LIMIT, OfflineStt, SttJob, decoder, max_utterance_samples,
};
pub use worker::{Job, MAX_THREADS, Worker, check_threads};
