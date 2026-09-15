use std::path::PathBuf;

use blogger_python::{bridge_init, ensure_python, scenarios};

fn scenarios_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/scenarios")
}

#[test]
fn python_embedding_smoke() {
    let tmp = std::env::temp_dir().join("blogger-python-test");
    bridge_init(&tmp, &scenarios_dir()).expect("bridge init");
    ensure_python().expect("python init");

    let v = scenarios::self_test().expect("self test");
    assert!(!v.is_empty(), "expected module version string");
    println!("blogger_automation version = {}", v[0]);

    let list = scenarios::list_scenarios().expect("list scenarios");
    println!("scenarios: {list}");
    assert!(!list.contains("\"failed to load\""), "a scenario failed to load:\n{list}");

    let run_env = scenarios::run_scenario("env_check", Some("{}")).expect("run env_check");
    println!("env_check result: {run_env}");
    assert!(run_env.contains("scenario_dir"));
}