//! El catálogo de `config/academy.toml`: se valida al arrancar y su vista pública
//! (`academy_catalog.v1`) nunca lleva las claves.

mod common;

use cfds_academy::{AcademyConfig, Catalog, ExamKind};
use common::{REAL, assert_valid, validator, with_questions};

#[test]
fn the_real_config_loads_with_levels_0_to_2_and_provisional_thresholds() {
    let cfg = AcademyConfig::from_toml(REAL).unwrap();
    let c = &cfg.catalog;
    assert_eq!(
        c.levels.iter().map(|l| l.level).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(cfg.lessons().count(), 10);
    assert!(c.thresholds.provisional);
    assert_eq!(c.thresholds.quiz_pass_pct.to_string(), "80");
    assert_eq!(c.catalog_version.len(), 64);
    // Sin preguntas todavía: los exámenes de preguntas no están listos; 0.3 no tiene examen.
    for l in cfg.lessons() {
        assert_eq!(l.exam.ready, l.exam.kind == ExamKind::None, "{}", l.id);
    }
    let watchlist = c.tools.iter().find(|t| t.id == "tool.watchlist").unwrap();
    assert_eq!(watchlist.lesson_id, "0.1");
    assert_valid(
        "academy_catalog.v1.schema.json",
        &serde_json::to_value(c).unwrap(),
    );
}

#[test]
fn the_catalog_roundtrips_and_never_carries_the_keys() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    let json = serde_json::to_value(&cfg.catalog).unwrap();
    assert_valid("academy_catalog.v1.schema.json", &json);
    let back: Catalog = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(back, cfg.catalog);
    let text = json.to_string();
    assert!(
        !text.contains("answer") && !text.contains("tolerance"),
        "{text}"
    );
    assert!(cfg.lesson("0.1").unwrap().exam.ready);
    // Un campo inventado o un null faltante: lo rechazan el contrato y Rust.
    let mut bad = json.clone();
    bad["levels"][0]["lessons"][0]["exam"]["questions"][0]["answer"] = "b".into();
    assert!(!validator("academy_catalog.v1.schema.json").is_valid(&bad));
    assert!(serde_json::from_value::<Catalog>(bad).is_err());
    let mut missing = json;
    missing["levels"][0]["lessons"][0]
        .as_object_mut()
        .unwrap()
        .remove("doc");
    assert!(!validator("academy_catalog.v1.schema.json").is_valid(&missing));
    assert!(serde_json::from_value::<Catalog>(missing).is_err());
}

#[test]
fn the_version_changes_with_any_change_of_the_config() {
    let a = AcademyConfig::from_toml(REAL).unwrap();
    let b =
        AcademyConfig::from_toml(&REAL.replace("quiz_pass_pct = \"80\"", "quiz_pass_pct = \"70\""))
            .unwrap();
    assert_ne!(a.catalog.catalog_version, b.catalog.catalog_version);
}

#[test]
fn invalid_configs_are_rejected_at_startup() {
    let base = with_questions();
    let cases: Vec<(&str, String)> = vec![
        ("umbral 0", base.replace("quiz_pass_pct = \"80\"", "quiz_pass_pct = \"0\"")),
        ("umbral > 100", base.replace("quiz_pass_pct = \"80\"", "quiz_pass_pct = \"101\"")),
        ("campo inventado", base.replace("provisional = true", "provisional = true\nextra = 1")),
        ("herramienta repetida", base.replace("id = \"tool.glossary\"", "id = \"tool.watchlist\"")),
        ("id de herramienta", base.replace("\"tool.glossary\"", "\"Tool.Glossary\"")),
        ("herramienta sin lección", base.replace("unlocks = [\"tool.glossary\"]", "unlocks = []")),
        ("dos lecciones abren la misma", base.replace("unlocks = [\"tool.glossary\"]", "unlocks = [\"tool.watchlist\"]")),
        ("desbloquea algo que no existe", base.replace("unlocks = []\n\n[[lessons]]\nid = \"1.1\"", "unlocks = [\"tool.nada\"]\n\n[[lessons]]\nid = \"1.1\"")),
        ("lección repetida", base.replace("id = \"0.2\"", "id = \"0.1\"")),
        ("lección de un nivel que no existe", base.replace("id = \"2.4\"", "id = \"3.1\"")),
        ("id de lección", base.replace("id = \"2.4\"", "id = \"2.04\"")),
        ("nivel repetido", base.replace("level = 2\n", "level = 1\n")),
        ("examen de replay", base.replace("exam = \"none\"", "exam = \"replay\"")),
        ("preguntas sin examen", base.replace("exam = \"quiz\"\nunlocks = [\"tool.watchlist\"", "exam = \"none\"\nunlocks = [\"tool.watchlist\"")),
        ("clave fuera de las opciones", base.replace("answer = \"b\"", "answer = \"z\"")),
        ("una sola opción", base.replace("options = [{ id = \"a\", text = \"El último acuerdo\" }, { id = \"b\", text = \"Una promesa\" }]", "options = [{ id = \"a\", text = \"El último acuerdo\" }]")),
        ("pregunta de otra lección", base.replace("id = \"0.1.2\"", "id = \"0.2.2\"")),
        ("pregunta repetida", base.replace("id = \"0.1.2\"", "id = \"0.1.1\"")),
        ("clave numérica que no es número", base.replace("answer = \"0.25\"", "answer = \"un cuarto\"")),
        ("tolerancia negativa", base.replace("tolerance = \"0.01\"", "tolerance = \"-0.01\"")),
        ("doc fuera de docs/academia", base.replace("title = \"Spread\"", "title = \"Spread\"\ndoc = \"../.env\"")),
    ];
    for (what, text) in cases {
        assert_ne!(text, base, "{what}: el reemplazo no cambió nada");
        assert!(
            AcademyConfig::from_toml(&text).is_err(),
            "{what}: debería rechazarse"
        );
    }
}
