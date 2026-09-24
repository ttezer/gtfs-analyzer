use gtfs_config::{merge_delta, ValidatorConfig};
use gtfs_core::{NameIndex, ValidateResult};
use gtfs_pipeline::validate_bytes;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

/// Validate one GTFS ZIP buffer using the shared Rust validation pipeline.
#[pyfunction]
#[pyo3(signature = (feed, today, config_json = "{}", include_name_index = false))]
fn validate(
    feed: &[u8],
    today: u32,
    config_json: &str,
    include_name_index: bool,
) -> PyResult<String> {
    let config =
        merge_delta(&ValidatorConfig::default(), config_json).map_err(PyValueError::new_err)?;
    let mut result = validate_bytes(feed, &config, today);

    if !include_name_index {
        if let ValidateResult::Ok(validation) = &mut result {
            validation.name_index = NameIndex::default();
        }
    }

    serde_json::to_string(&result)
        .map_err(|error| PyRuntimeError::new_err(format!("failed to serialize result: {error}")))
}

#[pymodule]
fn gtfs_analyzer(m: &Bound<'_, PyModule>) -> PyResult<()> {
    if std::env::var_os("GTFS_QUIET").is_none() {
        std::env::set_var("GTFS_QUIET", "1");
    }
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    Ok(())
}
