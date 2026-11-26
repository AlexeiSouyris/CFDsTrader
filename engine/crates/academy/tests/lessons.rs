//! El texto de las lecciones (`docs/academia/*.md`, #760): cada lección de la config real tiene
//! su archivo con el formato fijo, y cada cifra coincide con su fuente del repo (config, registro
//! de resultados o informe). Si cambia un spread, una tasa o un tope, este test falla hasta que la
//! lección (y sus preguntas) se corrijan.

mod common;

use std::path::{Path, PathBuf};

use cfds_academy::AcademyConfig;
use cfds_academy::catalog::{Exam, ExamKind, Lesson};
use cfds_academy::lesson_doc::{Evidence, FigureSource, parse_lesson_doc, verify_figures};
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
