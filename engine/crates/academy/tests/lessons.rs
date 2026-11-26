//! El texto de las lecciones (`docs/academia/*.md`, #760): cada lección de la config real tiene
//! su archivo con el formato fijo, y cada cifra coincide con su fuente del repo (config, registro
//! de resultados o informe). Si cambia un spread, una tasa o un tope, este test falla hasta que la
//! lección (y sus preguntas) se corrijan.

mod common;

use std::path::{Path, PathBuf};

use cfds_academy::catalog::{Exam, ExamKind, Lesson};
use cfds_academy::lesson_doc::{
    Evidence, FigureSource, MAX_DOC_BYTES, parse_lesson_doc, verify_figures,
};
use cfds_academy::{AcademyConfig, LessonDocs};
use common::REAL;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn read(rel: &str) -> Option<String> {
    std::fs::read_to_string(repo().join(rel)).ok()
}

#[test]
fn every_lesson_of_the_real_config_has_a_valid_text_with_verified_figures() {
    let cfg = AcademyConfig::from_toml(REAL).unwrap();
    let glossary = read("docs/GLOSARIO.md").unwrap();
    for lesson in cfg.lessons() {
        let path = lesson
            .doc
            .as_deref()
            .unwrap_or_else(|| panic!("{} sin doc", lesson.id));
        let md = read(path).unwrap_or_else(|| panic!("{path} no existe"));
        let doc = parse_lesson_doc(lesson, &md).unwrap_or_else(|e| panic!("{e}"));
        verify_figures(&lesson.id, &doc, &read).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            doc.figures
                .iter()
                .any(|f| matches!(f.source, FigureSource::Config { .. })
                    || matches!(f.source, FigureSource::Registry { .. })),
            "{}: al menos una cifra comparada con la config o el registro",
            lesson.id
        );
        for id in &doc.glossary {
            assert!(
                glossary.contains(&format!("<a id=\"{id}\"></a>")),
                "{}: el glosario no tiene el término {id}",
                lesson.id
            );
        }
        // Los enlaces relativos apuntan a archivos que existen.
        let dir = Path::new(path).parent().unwrap();
        for target in md.split("](").skip(1).filter_map(|r| r.split(')').next()) {
            if target.starts_with("http") || target.starts_with('#') {
                continue;
            }
            let file = target.split('#').next().unwrap();
            assert!(
                repo().join(dir).join(file).exists(),
                "{}: enlace roto {target}",
                lesson.id
            );
        }
    }
}

fn lesson() -> Lesson {
    Lesson {
        id: "2.1".into(),
        title: "Spread".into(),
        objective: "Saber que se entra pagando.".into(),
        doc: Some("docs/academia/x.md".into()),
        exam: Exam {
            kind: ExamKind::Quiz,
            ready: true,
            questions: vec![],
            replay_kind: None,
        },
        unlocks: vec![],
    }
}

const GOOD: &str = "# 2.1 Spread

> Educativo: no es consejo de inversión ni promete ganancias.

## Objetivo
Algo. Ver [spread](../GLOSARIO.md#spread).
## La idea
Texto.
## Interactivo
Algo.
## Ejercicio
Algo.
## Examen
5 preguntas.
## Desbloquea
Nada.
## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Spread US500 | 0,9 (`0.9`) | `config/instruments.toml` `instruments.US500.spread_typical_points` | A |
| ES por punto | `50` | `config/futures_contracts.toml` `contracts.ES.multiplier` | A |
| Fila del registro | `-0.19` | `docs/analysis/registro-de-resultados.csv` `R001` | A |
| Texto de un informe | `spreads fijos por franja` | `docs/research/ig-spreads-2026-11.md` | A |
| Externa | — | [ESMA](https://www.esma.europa.eu/) | B |
";

#[test]
fn a_well_formed_text_parses_and_its_figures_are_checked_against_the_repo() {
    let doc = parse_lesson_doc(&lesson(), GOOD).unwrap();
    assert_eq!(doc.figures.len(), 5);
    assert_eq!(doc.glossary, ["spread"]);
    assert_eq!(doc.figures[4].evidence, Evidence::B);
    assert!(matches!(
        doc.figures[4].source,
        FigureSource::External { ref url } if url == "https://www.esma.europa.eu/"
    ));
    verify_figures("2.1", &doc, &read).unwrap();
    // Una cifra que ya no coincide con su fuente falla, sea de config, registro o informe.
    for (from, to) in [
        ("(`0.9`)", "(`0.8`)"),
        ("| `50` |", "| `20` |"),
        ("| `-0.19` |", "| `-0.29` |"),
        ("`spreads fijos por franja`", "`spreads variables`"),
        (
            "`instruments.US500.spread_typical_points`",
            "`instruments.US500.no_existe`",
        ),
        ("`contracts.ES.multiplier`", "`contracts.ZZ.multiplier`"),
        ("`R001`", "`R999`"),
    ] {
        let md = GOOD.replace(from, to);
        assert_ne!(md, GOOD, "{to}");
        let doc = parse_lesson_doc(&lesson(), &md).unwrap();
        assert!(verify_figures("2.1", &doc, &read).is_err(), "{to}");
    }
}

#[test]
fn malformed_texts_are_rejected() {
    let cases = [
        (
            "otro título",
            GOOD.replace("# 2.1 Spread", "# 2.1 Diferencial"),
        ),
        (
            "sin aviso",
            GOOD.replace("no es consejo de inversión", "es educativo"),
        ),
        ("promete", GOOD.replace("Texto.", "Ganancias garantizadas.")),
        ("falta una sección", GOOD.replace("## Ejercicio\n", "")),
        (
            "fuera de orden",
            GOOD.replace("## Objetivo", "## X")
                .replace("## Desbloquea", "## Objetivo\n## Desbloquea"),
        ),
        ("cifras no al final", format!("{GOOD}\n## Notas\nAlgo.\n")),
        (
            "tabla sin encabezado",
            GOOD.replace("| Cifra | Valor | Fuente | Prueba |", "| a | b | c | d |"),
        ),
        ("prueba inválida", GOOD.replace("| B |", "| D |")),
        (
            "sin fuente",
            GOOD.replace("[ESMA](https://www.esma.europa.eu/)", "ESMA"),
        ),
        (
            "enlace no https",
            GOOD.replace("https://www.esma", "http://www.esma"),
        ),
        ("config sin valor", GOOD.replace("0,9 (`0.9`)", "0,9")),
        (
            "columnas de menos",
            GOOD.replace("| ES por punto | `50` |", "| ES por punto `50` |"),
        ),
        (
            "tabla vacía",
            GOOD.split("|---|---|---|---|").next().unwrap().to_owned() + "|---|---|---|---|\n",
        ),
    ];
    for (what, md) in cases {
        assert_ne!(md, GOOD, "{what}: el reemplazo no cambió nada");
        assert!(
            parse_lesson_doc(&lesson(), &md).is_err(),
            "{what}: debería rechazarse"
        );
    }
}

#[test]
fn the_real_texts_load_at_startup_and_are_served_by_lesson_id() {
    let cfg = AcademyConfig::from_toml(REAL).unwrap();
    let docs = LessonDocs::load(&cfg, &repo()).unwrap();
    assert_eq!(
        docs.len(),
        cfg.lessons().filter(|l| l.doc.is_some()).count()
    );
    let first = cfg.lessons().next().unwrap();
    let text = docs.get(&first.id).unwrap();
    assert_eq!(Some(text.doc.as_str()), first.doc.as_deref());
    assert_eq!(text.catalog_version, cfg.catalog.catalog_version);
    assert_eq!(Some(text.markdown.clone()), read(&text.doc));
    // El id es solo una clave: nada que parezca una ruta encuentra un texto.
    for odd in [
        "9.9",
        "..",
        "docs/academia/nivel-00-1-que-es-un-mercado.md",
        "",
    ] {
        assert!(docs.get(odd).is_none(), "{odd}");
    }
}

/// Una config mínima con la lección 2.1 (texto en `doc`, o sin texto con `None`).
fn one_lesson(doc: Option<&str>) -> AcademyConfig {
    let head = REAL.split("# Herramientas con candado").next().unwrap();
    let doc = doc.map_or(String::new(), |d| format!("doc = \"{d}\"\n"));
    let text = format!(
        "{head}[[tools]]\nid = \"tool.watchlist\"\nname = \"Watchlist\"\n\n\
         [[levels]]\nlevel = 2\ntitle = \"Costos\"\n\n\
         [[lessons]]\nid = \"2.1\"\ntitle = \"Spread\"\nobjective = \"Saber que se entra pagando.\"\n\
         {doc}exam = \"none\"\nunlocks = [\"tool.watchlist\"]\n"
    );
    AcademyConfig::from_toml(&text).unwrap()
}

/// Una raíz de repo de prueba (vacía) con `docs/academia/`.
fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("cfds-academy-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("docs/academia")).unwrap();
    root
}

#[test]
fn texts_load_only_from_docs_academia_and_only_when_valid() {
    let root = temp_root("load");
    let cfg = one_lesson(Some("docs/academia/x.md"));
    // Sin el archivo, no arranca.
    assert!(LessonDocs::load(&cfg, &root).is_err());
    std::fs::write(root.join("docs/academia/x.md"), GOOD).unwrap();
    let docs = LessonDocs::load(&cfg, &root).unwrap();
    assert_eq!(docs.get("2.1").unwrap().markdown, GOOD);
    // Un texto que no cumple el formato tampoco.
    std::fs::write(
        root.join("docs/academia/x.md"),
        GOOD.replace("## Ejercicio\n", ""),
    )
    .unwrap();
    assert!(LessonDocs::load(&cfg, &root).is_err());
    // Ni uno demasiado grande.
    let big = format!(
        "{GOOD}{}",
        "a".repeat(usize::try_from(MAX_DOC_BYTES).unwrap())
    );
    std::fs::write(root.join("docs/academia/x.md"), big).unwrap();
    assert!(LessonDocs::load(&cfg, &root).is_err());
    // Una carpeta con el nombre del texto no es un texto.
    std::fs::remove_file(root.join("docs/academia/x.md")).unwrap();
    std::fs::create_dir(root.join("docs/academia/x.md")).unwrap();
    assert!(LessonDocs::load(&cfg, &root).is_err());
    // Una lección sin texto no se sirve.
    let docs = LessonDocs::load(&one_lesson(None), &root).unwrap();
    assert!(docs.is_empty() && docs.get("2.1").is_none());
    // Sin la carpeta, no arranca.
    assert!(LessonDocs::load(&cfg, &root.join("otra")).is_err());
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn a_symlink_that_leaves_docs_academia_is_rejected() {
    let root = temp_root("symlink");
    std::fs::write(root.join("secreto.md"), GOOD).unwrap();
    std::os::unix::fs::symlink(root.join("secreto.md"), root.join("docs/academia/x.md")).unwrap();
    let cfg = one_lesson(Some("docs/academia/x.md"));
    let e = LessonDocs::load(&cfg, &root).unwrap_err().to_string();
    assert!(e.contains("sale de docs/academia"), "{e}");
    // Un enlace que queda adentro sí vale.
    std::fs::write(root.join("docs/academia/y.md"), GOOD).unwrap();
    std::fs::remove_file(root.join("docs/academia/x.md")).unwrap();
    std::os::unix::fs::symlink(
        root.join("docs/academia/y.md"),
        root.join("docs/academia/x.md"),
    )
    .unwrap();
    assert!(LessonDocs::load(&cfg, &root).is_ok());
    std::fs::remove_dir_all(&root).unwrap();
}
