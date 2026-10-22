//! Durable account profiles and folder configuration.
//! Credentials and authentication remain in the auth module.
mod models;
mod repository;
mod validation;

pub(crate) use repository::Accounts;

#[cfg(test)]
mod tests;
