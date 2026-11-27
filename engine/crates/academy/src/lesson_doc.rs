//! El texto de una lección (`docs/academia/*.md`, campo `doc` del catálogo), con un formato fijo
//! que el motor valida (#760). Funciones puras: el que llama lee los archivos.
//!
//! ```text
//! # <id> <título>                      igual que en config/academy.toml
//! > ... no es consejo de inversión ...  el aviso, siempre
//! ## Objetivo · ## Interactivo · ## Ejercicio · ## Examen · ## Desbloquea · ## Cifras y fuentes
//!                                       en ese orden (puede haber otras secciones en el medio)
//! | Cifra | Valor | Fuente | Prueba |    la tabla de la última sección: una fila por cifra
//! ```
//!
//! Cada fila dice de dónde sale la cifra y su nivel de prueba (A, B o C, §7 del diseño):
//! - `` `config/<archivo>.toml` `<clave.con.puntos>` ``: el motor compara el valor (el primer
//!   `` `código` `` de la columna Valor) con la config; si la config cambia, la lección falla.
//!   En un arreglo de tablas, el segmento elige la que tiene ese `id` (`contracts.ES.multiplier`).
//! - `` `docs/analysis/registro-de-resultados.csv` `R001` ``: el valor aparece en esa fila.
//! - `` `<otro archivo del repo>` `` (`config/`, `docs/`, `engine/`, `contracts/` o `CLAUDE.md`): el
//!   valor aparece escrito tal cual en el archivo.
//! - Un enlace `https://...`: fuente externa, verificada a mano (fecha en la fila).
//!
//! [`LessonDocs::load`] lee al arrancar el texto de cada lección del catálogo, **solo** dentro de
//! `docs/academia/`, para que el gateway lo sirva (`academy_lesson_doc.v1`, #798).

use std::collections::BTreeMap;
use std::path::Path;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::catalog::Lesson;
use crate::{AcademyConfig, AcademyError};

/// Las secciones obligatorias, en orden (§2 del diseño más las fuentes).
pub const SECTIONS: [&str; 6] = [
    "Objetivo",
    "Interactivo",
    "Ejercicio",
    "Examen",
    "Desbloquea",
    "Cifras y fuentes",
];

/// Frases que prometen ganancias (CLAUDE.md §5): ninguna lección las usa.
const PROMISES: [&str; 8] = [
    "ganancia garantizada",
    "ganancias garantizadas",
    "ganancia asegurada",
    "ganancias aseguradas",
    "rentabilidad garantizada",
    "sin riesgo de perder",
    "dinero fácil",
    "vas a ganar",
];

const REGISTRY: &str = "docs/analysis/registro-de-resultados.csv";

/// Nivel de prueba de una fuente (§7 del diseño): A = artículo revisado o dato oficial,
/// B = libro con método explícito, C = verificado en parte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    A,
    B,
    C,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FigureSource {
    /// Una clave de `config/*.toml`.
    Config { file: String, key: String },
    /// Una fila del registro de resultados.
    Registry { id: String },
    /// Otro archivo del repo donde la cifra está escrita tal cual.
    RepoFile { path: String },
    /// Fuente externa, verificada a mano.
    External { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Figure {
    pub what: String,
    /// El primer `` `código` `` de la columna Valor (obligatorio salvo en fuentes externas).
    pub value: Option<String>,
    pub source: FigureSource,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonDoc {
    pub figures: Vec<Figure>,
    /// Términos enlazados a `docs/GLOSARIO.md` (`../GLOSARIO.md#<id>`).
    pub glossary: Vec<String>,
}

macro_rules! ensure {
    ($cond:expr, $($msg:tt)+) => {
        if !$cond {
            return Err(AcademyError::LessonDoc(format!($($msg)+)));
        }
    };
}

/// Los `` `código` `` de un texto, en orden.
fn code_spans(s: &str) -> Vec<&str> {
    s.split('`').skip(1).step_by(2).collect()
}

fn repo_path_ok(p: &str) -> bool {
    (p == "CLAUDE.md"
        || ["config/", "docs/", "engine/", "contracts/"]
            .iter()
            .any(|d| p.starts_with(d)))
        && !p.contains("..")
        && !p.contains(' ')
}

fn figure(lesson: &str, row: &str) -> Result<Figure, AcademyError> {
    let cells: Vec<&str> = row
        .trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect();
    ensure!(
        cells.len() == 4 && !cells[0].is_empty(),
        "lección {lesson}: fila de cifras con 4 columnas (Cifra | Valor | Fuente | Prueba): {row}"
    );
    let evidence = match cells[3] {
        "A" => Evidence::A,
        "B" => Evidence::B,
        "C" => Evidence::C,
        other => {
            return Err(AcademyError::LessonDoc(format!(
                "lección {lesson}: prueba A, B o C, no «{other}» ({})",
                cells[0]
            )));
        }
    };
    let value = code_spans(cells[1]).first().map(|v| (*v).to_owned());
    let spans = code_spans(cells[2]);
    let source = match spans.as_slice() {
        [file, key, ..] if file.starts_with("config/") && file.ends_with(".toml") => {
            FigureSource::Config {
                file: (*file).to_owned(),
                key: (*key).to_owned(),
            }
        }
        [file, id, ..] if *file == REGISTRY => FigureSource::Registry {
            id: (*id).to_owned(),
        },
        [path, ..] if repo_path_ok(path) => FigureSource::RepoFile {
            path: (*path).to_owned(),
        },
        _ => {
            let url = cells[2]
                .split("](")
                .nth(1)
                .and_then(|r| r.split(')').next())
                .filter(|u| u.starts_with("https://"));
            match url {
                Some(url) => FigureSource::External {
                    url: url.to_owned(),
                },
                None => {
                    return Err(AcademyError::LessonDoc(format!(
                        "lección {lesson}: la cifra «{}» no tiene fuente (archivo del repo o enlace https)",
                        cells[0]
                    )));
                }
            }
        }
    };
    ensure!(
        value.is_some() || matches!(source, FigureSource::External { .. }),
        "lección {lesson}: la cifra «{}» necesita su valor en `código` para compararlo con la fuente",
        cells[0]
    );
    Ok(Figure {
        what: cells[0].to_owned(),
        value,
        source,
        evidence,
    })
}

/// Valida el texto `md` de `lesson`: título, aviso, secciones, sin promesas y la tabla de cifras.
pub fn parse_lesson_doc(lesson: &Lesson, md: &str) -> Result<LessonDoc, AcademyError> {
    let id = &lesson.id;
    let first = md
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default();
    ensure!(
        first == format!("# {id} {}", lesson.title),
        "lección {id}: la primera línea es «# {id} {}»",
        lesson.title
    );
    let lower = md.to_lowercase();
    ensure!(
        lower.contains("no es consejo de inversión"),
        "lección {id}: falta el aviso «no es consejo de inversión»"
    );
    if let Some(p) = PROMISES.iter().find(|p| lower.contains(*p)) {
        return Err(AcademyError::LessonDoc(format!(
            "lección {id}: promete ganancias («{p}», CLAUDE.md §5)"
        )));
    }
    let headings: Vec<&str> = md.lines().filter_map(|l| l.strip_prefix("## ")).collect();
    let mut at = 0;
    for s in SECTIONS {
        let found = headings[at..].iter().position(|h| h.trim() == s);
        ensure!(
            found.is_some(),
            "lección {id}: falta la sección «## {s}» (o está fuera de orden)"
        );
        at += found.unwrap_or_default() + 1;
    }
    ensure!(
        at == headings.len(),
        "lección {id}: «## Cifras y fuentes» es la última sección"
    );
    let table = md.split("\n## Cifras y fuentes").nth(1).unwrap_or_default();
    let rows: Vec<&str> = table
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('|'))
        .collect();
    ensure!(
        rows.first()
            .is_some_and(|h| h.replace(' ', "") == "|Cifra|Valor|Fuente|Prueba|")
            && rows.get(1).is_some_and(|s| s.starts_with("|---")),
        "lección {id}: la tabla de cifras empieza con | Cifra | Valor | Fuente | Prueba |"
    );
    let figures = rows[2..]
        .iter()
        .map(|r| figure(id, r))
        .collect::<Result<Vec<_>, _>>()?;
    ensure!(
        !figures.is_empty(),
        "lección {id}: la tabla de cifras está vacía"
    );
    let glossary = md
        .split("](../GLOSARIO.md#")
        .skip(1)
        .filter_map(|r| r.split(')').next())
        .map(str::to_owned)
        .collect();
    Ok(LessonDoc { figures, glossary })
}

/// El valor de `key` en un TOML (`a.b.c`; en un arreglo de tablas, la que tiene ese `id`). Si es
/// una tabla con `value` (`{ value = "0.9", source = ... }`), ese valor. Texto o entero.
fn toml_value(text: &str, key: &str) -> Option<String> {
    let mut v: toml::Value = toml::from_str::<toml::Table>(text).ok()?.into();
    for seg in key.split('.') {
        v = match v {
            toml::Value::Table(mut t) => t.remove(seg)?,
            toml::Value::Array(a) => a
                .into_iter()
                .find(|e| e.get("id").and_then(toml::Value::as_str) == Some(seg))?,
            _ => return None,
        };
    }
    if let toml::Value::Table(mut t) = v {
        v = t.remove("value")?;
    }
    match v {
        toml::Value::String(s) => Some(s),
        toml::Value::Integer(i) => Some(i.to_string()),
        toml::Value::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Mismo número (`"3.0"` = `"3"`) o, si no son números, el mismo texto.
fn same(a: &str, b: &str) -> bool {
    match (a.parse::<Decimal>(), b.parse::<Decimal>()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Compara cada cifra con su fuente del repo. `read` devuelve el texto de un archivo del repo
/// (ruta relativa a la raíz), o `None` si no existe. Las fuentes externas solo se revisan a mano.
pub fn verify_figures(
    lesson: &str,
    doc: &LessonDoc,
    read: &dyn Fn(&str) -> Option<String>,
) -> Result<(), AcademyError> {
    for f in &doc.figures {
        let value = f.value.as_deref().unwrap_or_default();
        let what = &f.what;
        match &f.source {
            FigureSource::Config { file, key } => {
                let found = read(file).and_then(|t| toml_value(&t, key));
                ensure!(
                    found.as_deref().is_some_and(|c| same(c, value)),
                    "lección {lesson}: «{what}» = {value}, pero {file} {key} = {found:?}"
                );
            }
            FigureSource::Registry { id } => {
                let row = read(REGISTRY).and_then(|t| {
                    t.lines()
                        .find(|l| l.starts_with(&format!("{id},")))
                        .map(str::to_owned)
                });
                ensure!(
                    row.as_deref().is_some_and(|r| r.contains(value)),
                    "lección {lesson}: «{what}» = {value} no está en la fila {id} del registro"
                );
            }
            FigureSource::RepoFile { path } => {
                ensure!(
                    read(path).is_some_and(|t| t.contains(value)),
                    "lección {lesson}: «{what}» = {value} no aparece en {path}"
                );
            }
            FigureSource::External { .. } => {}
        }
    }
    Ok(())
}

/// La carpeta de los textos, relativa a la raíz del repo. El campo `doc` del catálogo empieza así.
pub const DOCS_DIR: &str = "docs/academia";

/// Tamaño máximo de un texto (el mismo tope que `academy_lesson_doc.v1`).
pub const MAX_DOC_BYTES: u64 = 256 * 1024;

/// `contracts/academy_lesson_doc.v1`: el texto de una lección, tal como está en el repo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LessonText {
    pub lesson_id: String,
    pub doc: String,
    pub catalog_version: String,
    pub markdown: String,
}

/// El texto de cada lección con `doc`, leído y validado al arrancar. El gateway lo busca por el
/// id de la lección: el pedido nunca elige un archivo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LessonDocs(BTreeMap<String, LessonText>);

impl LessonDocs {
    /// Lee de `root` (la raíz del repo) el texto de cada lección del catálogo que tiene `doc`.
    /// Falla si alguno no está, sale de `docs/academia/` (también con un enlace simbólico), pasa
    /// de [`MAX_DOC_BYTES`], no es UTF-8 o no cumple el formato ([`parse_lesson_doc`]).
    pub fn load(cfg: &AcademyConfig, root: &Path) -> Result<Self, AcademyError> {
        let err = |m: String| AcademyError::LessonDoc(m);
        let base = root
            .join(DOCS_DIR)
            .canonicalize()
            .map_err(|e| err(format!("{DOCS_DIR}: {e}")))?;
        let version = &cfg.catalog.catalog_version;
        let mut out = BTreeMap::new();
        for lesson in cfg.lessons() {
            let Some(doc) = lesson.doc.as_deref() else {
                continue;
            };
            let id = &lesson.id;
            let inside = doc
                .strip_prefix(DOCS_DIR)
                .is_some_and(|r| r.starts_with('/') && !r.contains(".."));
            ensure!(inside, "lección {id}: {doc} no está en {DOCS_DIR}/");
            let file = root
                .join(doc)
                .canonicalize()
                .map_err(|e| err(format!("lección {id}: {doc}: {e}")))?;
            ensure!(
                file.starts_with(&base),
                "lección {id}: {doc} sale de {DOCS_DIR}/"
            );
            let meta = std::fs::metadata(&file).map_err(|e| err(format!("{doc}: {e}")))?;
            ensure!(
                meta.is_file() && meta.len() <= MAX_DOC_BYTES,
                "lección {id}: {doc} no es un archivo o pasa de {MAX_DOC_BYTES} bytes"
            );
            let markdown =
                std::fs::read_to_string(&file).map_err(|e| err(format!("{doc}: {e}")))?;
            parse_lesson_doc(lesson, &markdown)?;
            let text = LessonText {
                lesson_id: id.clone(),
                doc: doc.to_owned(),
                catalog_version: version.clone(),
                markdown,
            };
            out.insert(id.clone(), text);
        }
        Ok(Self(out))
    }

    /// El texto de `lesson_id`, o `None` si la lección no existe o no tiene texto.
    pub fn get(&self, lesson_id: &str) -> Option<&LessonText> {
        self.0.get(lesson_id)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
