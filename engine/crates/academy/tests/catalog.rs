//! El catálogo de `config/academy.toml`: se valida al arrancar y su vista pública
//! (`academy_catalog.v1`) nunca lleva las claves.

mod common;

use cfds_academy::replay::ReplayKind;
use cfds_academy::{AcademyConfig, Catalog, ExamKind};
use common::{REAL, assert_valid, validator, with_questions, without_questions};

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
    // Con el contenido de los niveles 0–2 (#760): todos los exámenes están listos, cada examen de
    // preguntas tiene al menos 5 y cada lección tiene su texto.
    for l in cfg.lessons() {
        assert!(l.exam.ready, "{}", l.id);
        assert!(l.doc.is_some(), "{}", l.id);
        if l.exam.kind == ExamKind::Quiz {
            assert!(l.exam.questions.len() >= 5, "{}", l.id);
        }
    }
    // Sin sus preguntas, un examen de preguntas no está listo (0.3 no tiene examen).
    let bare = AcademyConfig::from_toml(&without_questions(REAL)).unwrap();
    for l in bare.lessons() {
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
        ("replay sin su tipo", base.replace("exam = \"none\"", "exam = \"replay\"")),
        ("replay de práctica", base.replace("exam = \"none\"", "exam = \"replay\"\nreplay_kind = \"practice\"")),
        ("tipo de replay sin replay", base.replace("exam = \"none\"", "exam = \"none\"\nreplay_kind = \"levels\"")),
        ("tipo de replay que no existe", base.replace("exam = \"none\"", "exam = \"replay\"\nreplay_kind = \"exam\"")),
        ("preguntas sin examen", base.replace("exam = \"quiz\"\nunlocks = [\"tool.watchlist\"", "exam = \"none\"\nunlocks = [\"tool.watchlist\"")),
        ("clave fuera de las opciones", base.replace("answer = \"b\"", "answer = \"z\"")),
        ("una sola opción", base.replace("options = [{ id = \"a\", text = \"El último acuerdo\" }, { id = \"b\", text = \"Una promesa\" }]", "options = [{ id = \"a\", text = \"El último acuerdo\" }]")),
        ("pregunta de otra lección", base.replace("id = \"0.1.2\"", "id = \"0.2.2\"")),
        ("pregunta repetida", base.replace("id = \"0.1.2\"", "id = \"0.1.1\"")),
        ("clave numérica que no es número", base.replace("answer = \"0.25\"", "answer = \"un cuarto\"")),
        ("tolerancia negativa", base.replace("tolerance = \"0.01\"", "tolerance = \"-0.01\"")),
        ("doc fuera de docs/academia", base.replace("doc = \"docs/academia/nivel-02-1-spread.md\"", "doc = \"../.env\"")),
        ("doc que no es .md", base.replace("nivel-02-1-spread.md", "nivel-02-1-spread.txt")),
    ];
    for (what, text) in cases {
        assert_ne!(text, base, "{what}: el reemplazo no cambió nada");
        assert!(
            AcademyConfig::from_toml(&text).is_err(),
            "{what}: debería rechazarse"
        );
    }
}

#[test]
fn a_replay_exam_is_ready_and_says_its_session_kind() {
    let cfg = AcademyConfig::from_toml(&common::with_replay_exam()).unwrap();
    let exam = &cfg.lesson("1.1").unwrap().exam;
    assert_eq!(
        (exam.kind, exam.ready, exam.replay_kind),
        (ExamKind::Replay, true, Some(ReplayKind::Levels))
    );
    assert!(exam.questions.is_empty());
    let json = serde_json::to_value(&cfg.catalog).unwrap();
    let v = validator("academy_catalog.v1.schema.json");
    assert!(v.is_valid(&json), "{json}");
    assert_eq!(
        json["levels"][1]["lessons"][0]["exam"]["replay_kind"],
        "levels"
    );
    assert_eq!(
        json["levels"][0]["lessons"][0]["exam"]["replay_kind"],
        serde_json::Value::Null
    );
    // El contrato pide el tipo en un examen de replay, y solo en él.
    let mut no_kind = json.clone();
    no_kind["levels"][1]["lessons"][0]["exam"]["replay_kind"] = serde_json::Value::Null;
    let mut stray = json.clone();
    stray["levels"][0]["lessons"][0]["exam"]["replay_kind"] = "trades".into();
    for bad in [no_kind, stray] {
        assert!(!v.is_valid(&bad));
    }
    // Un catálogo de antes de #759 (sin el campo) sigue valiendo.
    let mut old = AcademyConfig::from_toml(&with_questions())
        .map(|c| serde_json::to_value(&c.catalog).unwrap())
        .unwrap();
    for level in old["levels"].as_array_mut().unwrap() {
        for lesson in level["lessons"].as_array_mut().unwrap() {
            lesson["exam"]
                .as_object_mut()
                .unwrap()
                .remove("replay_kind");
        }
    }
    assert!(v.is_valid(&old));
    assert!(serde_json::from_value::<Catalog>(old).is_ok());
}
