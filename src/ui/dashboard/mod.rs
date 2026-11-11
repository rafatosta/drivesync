//! Main application destinations and startup presentation.
mod pages;
pub(crate) mod restoration;
pub(crate) use pages::overview::build;
mod components;
mod controller;
pub(crate) mod window;
