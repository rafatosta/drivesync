//! Root-bound local metadata collection, independent of event delivery and watchers.

mod collector;
mod filesystem;
mod metadata;
mod models;

#[cfg(test)]
mod tests;
