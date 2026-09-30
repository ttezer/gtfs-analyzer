use gtfs_config::{merge_delta, ValidatorConfig};
use gtfs_core::i18n::{Lang, Translator};
use gtfs_core::{NameIndex, ValidateResult};
use gtfs_pipeline::validate_bytes;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

/// Validate one GTFS ZIP buffer using the shared Rust validation pipeline.
#[pyfunction]
#[pyo3(signature = (feed, today, config_json = "{}", include_name_index = false, lang = "en"))]
fn validate(
    feed: &[u8],
    today: u32,
    config_json: &str,
    include_name_index: bool,
    lang: &str,
) -> PyResult<String> {
    // Dil, doğrulamadan ÖNCE denetlenir: geçersiz bir kod feed işlendikten sonra değil,
    // hemen ValueError olmalı.
    let lang: Lang = lang.parse().map_err(PyValueError::new_err)?;
    let translator = Translator::new(lang).map_err(PyRuntimeError::new_err)?;
    let config =
        merge_delta(&ValidatorConfig::default(), config_json).map_err(PyValueError::new_err)?;
    let mut result = validate_bytes(feed, &config, today);

    // CLI ile aynı çeviri (crates/cli/src/main.rs): title/message/remediation ve fatal.
    if let Some(translator) = &translator {
        match &mut result {
            ValidateResult::Ok(validation) => {
                for notice in &mut validation.notices {
                    translator.translate(notice);
                }
            }
            ValidateResult::Fatal(err) => translator.translate_fatal(err),
        }
    }

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
    // CARGO_PKG_VERSION = kök Cargo.toml [workspace.package] version; `__init__.py` buradan okur.
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
