use ic_agent::export::Principal;
use ic_agent::Agent;

/// An implementation for HTTP requests via `reqwest`.
pub struct Remote {
    agent: Agent,
    canister_id: Principal,
    /// A worker thread which performs the actual request.
    handle: Option<std::thread::JoinHandle<Result<(), remote::Error>>>,
    /// A channel to send requests (work) to the worker thread.
    request: std::sync::mpsc::SyncSender<remote::Request>,
    /// A channel to receive the result of the prior request.
    response: std::sync::mpsc::Receiver<remote::Response>,
}

///
mod remote;
