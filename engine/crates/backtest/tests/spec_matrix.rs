//! La matriz de trazabilidad de la Fase 4 (`docs/research/fase-4-trazabilidad.md`) no puede
//! quedar vieja (#87):
//! - cada test que nombra tiene que existir;
//! - cada fila de regla nombra un test (✅), su Issue (⏳) o la fase a la que pertenece (➡️).

use std::path::Path;

const MATRIX: &str = include_str!("../../../../docs/research/fase-4-trazabilidad.md");

/// Referencias `` `crates/…/archivo.rs::nombre` `` de una línea.
fn references(line: &str) -> Vec<(&str, &str)> {
    line.split('`')
        .skip(1)
        .step_by(2)
        .filter_map(|code| code.split_once("::"))
        .filter(|(path, _)| path.starts_with("crates/") && path.ends_with(".rs"))
        .collect()
}

/// Filas de las tablas de reglas: las de las secciones §, sin el encabezado ni el separador.
fn rule_rows() -> Vec<&'static str> {
    let mut rows = Vec::new();
    let mut in_rules = false;
    for line in MATRIX.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            in_rules = title.starts_with('§');
        } else if in_rules
            && line.starts_with('|')
            && !line.starts_with("| Regla")
            && !line.starts_with("|---")
        {
            rows.push(line);
        }
    }
    rows
}

#[test]
fn every_named_test_exists() {
    let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut missing = Vec::new();
    let mut count = 0;
    for line in MATRIX.lines() {
        for (path, name) in references(line) {
            count += 1;
            let src = std::fs::read_to_string(engine.join(path)).unwrap_or_default();
            let declared = src.lines().any(|l| {
                l.trim_start()
                    .strip_prefix("fn ")
                    .and_then(|rest| rest.strip_prefix(name))
                    .is_some_and(|rest| rest.starts_with('('))
            });
            if !declared {
                missing.push(format!("{path}::{name}"));
            }
        }
    }
    assert!(count > 50, "la matriz nombra solo {count} tests");
    assert!(missing.is_empty(), "tests que no existen: {missing:#?}");
}

#[test]
fn every_rule_names_a_test_or_an_issue() {
    let rows = rule_rows();
    assert!(rows.len() > 50, "solo {} reglas", rows.len());
    // ⏳ nombra su Issue de la fase 4; ➡️ nombra la fase a la que pertenece.
    let orphans: Vec<_> = rows
        .iter()
        .filter(|row| {
            references(row).is_empty()
                && !(row.contains("⏳") && row.contains('#'))
                && !(row.contains("➡️") && row.contains("Fase "))
        })
        .collect();
    let without_status: Vec<_> = rows
        .iter()
        .filter(|row| !["✅", "⏳", "➡️"].iter().any(|s| row.contains(s)))
        .collect();
    assert!(orphans.is_empty(), "reglas sin test ni Issue: {orphans:#?}");
    assert!(
        without_status.is_empty(),
        "reglas sin estado: {without_status:#?}"
    );
}

#[test]
fn a_covered_rule_always_names_a_test() {
    let bad: Vec<_> = rule_rows()
        .into_iter()
        .filter(|row| row.contains("✅") && references(row).is_empty())
        .collect();
    assert!(bad.is_empty(), "marcadas ✅ sin test: {bad:#?}");
}
