mod bindings;
mod bridge;
mod error;
pub mod scenarios;

pub use bindings::{BlogInfo, BlogPost, OAuthState, TabInfo};
pub use bridge::{BridgeCore, ChromeService, block_on, core as bridge_core, init as bridge_init};
pub use error::{BridgeError, Result};

use pyo3::prelude::*;
use pyo3::types::PyModule;

#[pymodule]
fn blogger_automation(m: &Bound<'_, PyModule>) -> PyResult<()> {
    bindings::register(m)
}

// Register the extension module with the embedded interpreter so that scenario
// scripts can simply `import blogger_automation`. Must run before any Python
// interpreter is initialised.
fn register_inittab() {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| {
        pyo3::append_to_inittab!(blogger_automation);
    });
}

/// Ensure the embedded interpreter is usable (idempotent).
pub fn ensure_python() -> Result<()> {
    register_inittab();
    pyo3::Python::initialize();
    pyo3::Python::attach(|py| {
        py.import("blogger_automation")?;
        Ok(())
    })
    .map_err(|e: PyErr| BridgeError::other(e.to_string()))
}