#[cfg(feature = "async-network-client")]
mod async_io;

#[cfg(feature = "blocking-network-client")]
mod blocking_io;

pub struct Fixture<'a>(pub &'a [u8]);
