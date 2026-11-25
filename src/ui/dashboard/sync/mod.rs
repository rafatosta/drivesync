//! Independently controlled GTK components and replaceable presentation backend.
// Public component states intentionally precede functional integration of later events.
#![allow(dead_code)]
pub mod backend;
pub mod components;
pub mod controller;
pub mod models;
mod recent_activity;
#[cfg(test)]
mod tests;
