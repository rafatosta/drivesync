//! Internal detection components; no public detection facade is implemented yet.

// The isolated collector has no production caller until the local detector lands.
#[allow(
    dead_code,
    reason = "The collector is intentionally private until detector composition exists"
)]
mod local;
