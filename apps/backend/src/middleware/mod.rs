//! Middleware components for the LCARS backend.

mod auth;

pub use auth::{auth_middleware, require_admin};
