//! Frontend auth state, signed-request client, and route guards.

pub mod client;
pub mod state;

pub use client::ApiClient;
pub use state::{AuthAction, AuthContext, AuthProvider, AuthState, Principal, use_auth};
