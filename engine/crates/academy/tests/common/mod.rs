//! Soporte de los tests: los contratos con sus `$ref` resueltos desde el repo, y una config de
//! ejemplo con preguntas fijas (la real, sin sus preguntas, más unas de prueba): así los tests de
//! corrección no dependen del contenido de las lecciones (#760).

#![allow(dead_code)]

use std::path::PathBuf;

use serde_json::Value;

pub const REAL: &str = include_str!("../../../../../config/academy.toml");

/// `text` sin ningún bloque `[[lessons.questions]]` (cada uno llega hasta la tabla siguiente que
/// no es una pregunta, o al final).
pub fn without_questions(text: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in text.lines() {
        if line == "[[lessons.questions]]" {
            skipping = true;
        } else if line.starts_with('[') {
            skipping = false;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// La config real sin sus preguntas, más preguntas de prueba en 0.1 (choice) y 2.1 (number).
pub fn with_questions() -> String {
    let mut text = without_questions(REAL);
    let q01 = r#"
[[lessons.questions]]
id = "0.1.1"
kind = "choice"
prompt = "¿Quién está del otro lado de tu operación?"
options = [{ id = "a", text = "Nadie" }, { id = "b", text = "Otra parte" }, { id = "c", text = "El gráfico" }]
answer = "b"

[[lessons.questions]]
id = "0.1.2"
kind = "choice"
prompt = "¿Qué es el precio?"
options = [{ id = "a", text = "El último acuerdo" }, { id = "b", text = "Una promesa" }]
answer = "a"
"#;
    let q21 = r#"
[[lessons.questions]]
id = "2.1.1"
kind = "number"
prompt = "Spread de 1 punto y stop de 4 puntos: ¿cuántos R es el spread?"
answer = "0.25"
tolerance = "0.01"
unit = "R"
"#;
    for (after, block) in [
        (
            "unlocks = [\"tool.watchlist\", \"tool.instrument_panel\"]\n",
            q01,
        ),
        ("unlocks = [\"tool.ticket_costs\"]\n", q21),
    ] {
        let at = text.find(after).expect("la config real cambió") + after.len();
        text.insert_str(at, block);
    }
    text
}

/// `with_questions()` con la lección 1.1 (abre `tool.source_selector`) como examen de replay de
/// niveles (#759).
pub fn with_replay_exam() -> String {
    let from = "exam = \"quiz\"\nunlocks = [\"tool.source_selector\"]";
    let text = with_questions();
    assert!(text.contains(from), "la config real cambió");
    text.replace(
        from,
        "exam = \"replay\"\nreplay_kind = \"levels\"\nunlocks = [\"tool.source_selector\"]",
    )
}

/// Un `exam_result.v1` de una sesión de niveles de `lesson` (o una práctica sin lección).
pub fn replay_result(
    lesson: Option<&str>,
    passed: Option<bool>,
    version: &str,
) -> cfds_academy::grade::ExamResult {
    serde_json::from_value(serde_json::json!({
        "session_id": "rp-00000000000000aa",
        "kind": if passed.is_some() { "levels" } else { "practice" },
        "lesson_id": lesson,
        "catalog_version": version,
        "passed": passed,
        "rules": [{"rule": "level_precision", "passed": passed == Some(true)}],
        "trades": [],
        "levels": null,
        "revealed": {
            "instrument_id": "US500",
            "timeframe": "1h",
            "source": "futures",
            "first_bar_at": "2019-03-12T13:00:00Z",
            "last_bar_at": "2019-04-30T19:00:00Z"
        },
        "graded_at": "2026-11-26T21:00:00Z"
    }))
    .unwrap()
}

fn contracts() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts")
}

/// Resuelve los `$ref` entre contratos con los archivos del repo, sin red.
struct Local;

impl jsonschema::Retrieve for Local {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let name = uri.as_str().rsplit('/').next().ok_or("uri sin archivo")?;
        let text = std::fs::read_to_string(contracts().join(name))?;
        Ok(serde_json::from_str(&text)?)
    }
}

pub fn validator(schema_file: &str) -> jsonschema::Validator {
    let text = std::fs::read_to_string(contracts().join(schema_file)).unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap_or_else(|e| panic!("{schema_file} no es un schema válido: {e}"))
}

/// Un sub-schema (`$defs/<name>`) del contrato.
pub fn def_validator(schema_file: &str, def: &str) -> jsonschema::Validator {
    let text = std::fs::read_to_string(contracts().join(schema_file)).unwrap();
    let mut schema: Value = serde_json::from_str(&text).unwrap();
    let defs = schema["$defs"].clone();
    let mut sub = defs[def].clone();
    sub["$defs"] = defs;
    sub["$id"] = schema["$id"].take();
    jsonschema::options()
        .with_retriever(Local)
        .build(&sub)
        .unwrap()
}

pub fn assert_valid(schema_file: &str, doc: &Value) {
    let errors: Vec<String> = validator(schema_file)
        .iter_errors(doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{schema_file} rechazó {doc}: {errors:?}");
}
