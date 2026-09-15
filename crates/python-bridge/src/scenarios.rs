use std::ffi::CString;

use pyo3::prelude::*;

use crate::bridge::core;
use crate::error::{BridgeError, Result};

/// Bare-bones scenario runner. Scenario scripts live in `scenarios_dir`,
/// are plain `.py` files exposing `META` (dict) and `scenario(params)`.
const BOOTSTRAP: &str = r#"
import os, sys, json, importlib.util

def _bp_scenario_path(name):
    root = os.environ.get('BLOGGER_SCENARIOS', '.')
    base = name if name.endswith('.py') else name + '.py'
    return os.path.join(root, base)

def _bp_scenario_load(name):
    path = _bp_scenario_path(name)
    if not os.path.exists(path):
        raise FileNotFoundError('scenario not found: ' + path)
    spec = importlib.util.spec_from_file_location('scenario_' + name.replace('.', '_'), path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = mod
    spec.loader.exec_module(mod)
    return mod

def __bp_scenario_run(name, params_json):
    mod = _bp_scenario_load(name)
    fn = getattr(mod, 'scenario', None)
    if fn is None:
        raise AttributeError('scenario "%s" has no scenario(params) function' % name)
    params = json.loads(params_json) if params_json else {}
    result = fn(params)
    return json.dumps(result, default=str, ensure_ascii=False)

def __bp_scenario_list():
    root = os.environ.get('BLOGGER_SCENARIOS', '.')
    out = []
    if not os.path.isdir(root):
        return json.dumps(out)
    for f in sorted(os.listdir(root)):
        if f.startswith('_') or not f.endswith('.py'):
            continue
        name = f[:-3]
        try:
            mod = _bp_scenario_load(name)
            meta = getattr(mod, 'META', {})
            if not isinstance(meta, dict):
                meta = {}
            doc = (getattr(mod, '__doc__', '') or '').strip()
            out.append({
                'name': meta.get('name', name),
                'description': meta.get('description', doc),
                'file': f,
                'params': meta.get('params', []),
            })
        except Exception as exc:
            out.append({'name': name, 'description': 'failed to load: %s' % exc, 'file': f})
    return json.dumps(out, ensure_ascii=False)
"#;

fn bootstrap_code() -> String {
    let dir = core().map(|c| c.scenarios_dir.clone()).unwrap_or_default();
    let dir_js = serde_json::to_string(&dir.to_string_lossy().to_string()).unwrap_or_default();
    format!(
        "import os\nos.environ['BLOGGER_SCENARIOS'] = {dir_js}\n{BOOTSTRAP}"
    )
}

pub fn list_scenarios() -> Result<String> {
    crate::ensure_python()?;
    Python::attach(|py| -> PyResult<String> {
        let code = CString::new(bootstrap_code()).map_err(py_err)?;
        py.run(code.as_c_str(), None, None)?;
        let expr = CString::new("__bp_scenario_list()").map_err(py_err)?;
        let out = py.eval(expr.as_c_str(), None, None)?;
        out.extract::<String>()
    })
    .map_err(|e: PyErr| BridgeError::other(e.to_string()))
}

pub fn run_scenario(name: &str, params_json: Option<&str>) -> Result<String> {
    crate::ensure_python()?;
    Python::attach(|py| -> PyResult<String> {
        let code = CString::new(bootstrap_code()).map_err(py_err)?;
        py.run(code.as_c_str(), None, None)?;
        let name_js = serde_json::to_string(name)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let params_js = serde_json::to_string(params_json.unwrap_or("{}"))
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let expr_str = format!("__bp_scenario_run({name_js}, {params_js})");
        let expr = CString::new(expr_str).map_err(py_err)?;
        let out = py.eval(expr.as_c_str(), None, None)?;
        out.extract::<String>()
    })
    .map_err(|e: PyErr| BridgeError::other(e.to_string()))
}

fn py_err(e: std::ffi::NulError) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(e.to_string())
}

/// Quick sanity check that our module loads from python side.
pub fn self_test() -> Result<Vec<String>> {
    Python::attach(|py| -> PyResult<Vec<String>> {
        let m = py.import("blogger_automation")?;
        let f = m.getattr("version")?;
        let v: String = f.call0()?.extract()?;
        let _ = m.getattr("blogger_list_blogs")?;
        Ok(vec![v])
    })
    .map_err(|e: PyErr| BridgeError::other(e.to_string()))
}