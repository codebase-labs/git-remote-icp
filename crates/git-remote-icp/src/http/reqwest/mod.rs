// https://github.com/Byron/gitoxide/blob/e6b9906c486b11057936da16ed6e0ec450a0fb83/git-transport/src/client/blocking_io/http/reqwest/mod.rs

/// An implementation for HTTP requests via `reqwest`.
pub struct Remote {
    /// A worker thread which performs the actual request.
    handle: Option<std::thread::JoinHandle<Result<(), remote::Error>>>,
    /// A channel to send requests (work) to the worker thread.
    request: std::sync::mpsc::SyncSender<remote::Request>,
    /// A channel to receive the result of the prior request.
    response: std::sync::mpsc::Receiver<remote::Response>,
}

///
mod remote;
