//! Guardia de los tests que escriben en PostgreSQL (#376): **nunca** en la base real.
//!
//! Los tests de integración (`#[ignore]`, el CI los corre con `-- --ignored`) leen la base de
//! [`test_database_url`] en vez de `DATABASE_URL` directo. Si el nombre de la base no empieza con
//! [`TEST_DB_PREFIX`] (`cfds_test`), el test falla antes de conectarse, con un mensaje claro.
//! Así correrlos con el `DATABASE_URL` del `.env` (la base real) no escribe nada.
//!
//! Es código de soporte de tests: el motor no lo usa en producción.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Prefijo obligatorio del nombre de la base de los tests (`cfds_test`, `cfds_test_<agente>`…).
pub const TEST_DB_PREFIX: &str = "cfds_test";

/// El nombre de la base de una URL de PostgreSQL (`postgres://u:p@host:5432/<base>?…`).
/// `None` si la URL no tiene base.
pub fn database_name(url: &str) -> Option<&str> {
    let base = url.split_once('?').map_or(url, |(b, _)| b);
    let (_, after_scheme) = base.split_once("://")?;
    let (_, db) = after_scheme.split_once('/')?;
    (!db.is_empty()).then_some(db)
}

/// La misma URL, apuntando a otra base del mismo servidor (conserva usuario y parámetros).
///
/// # Panics
/// Si la URL no tiene nombre de base.
pub fn with_database(url: &str, db: &str) -> String {
    let (base, query) = url
        .split_once('?')
        .map_or((url, None), |(b, q)| (b, Some(q)));
    let (server, _) = base
        .rsplit_once('/')
        .expect("DATABASE_URL sin nombre de base");
    match query {
        Some(q) => format!("{server}/{db}?{q}"),
        None => format!("{server}/{db}"),
    }
}

/// Revisa que la URL apunte a una base de pruebas. El error explica qué hacer.
pub fn check_test_database(url: &str) -> Result<(), String> {
    match database_name(url) {
        Some(db) if db.starts_with(TEST_DB_PREFIX) => Ok(()),
        Some(db) => Err(format!(
            "los tests no escriben en la base real: DATABASE_URL apunta a la base {db:?}. \
             Usá una base cuyo nombre empiece con {TEST_DB_PREFIX:?} (por ejemplo \
             cfds_test_<tu-nombre>, creada con createdb) o dejá estos tests para el CI (#376)"
        )),
        None => Err(format!(
            "DATABASE_URL sin nombre de base: los tests necesitan una base cuyo nombre empiece \
             con {TEST_DB_PREFIX:?} (#376)"
        )),
    }
}

/// `DATABASE_URL`, solo si apunta a una base de pruebas (`cfds_test…`).
///
/// # Panics
/// Si falta `DATABASE_URL` o si su base no es de pruebas: el test falla sin conectarse.
pub fn test_database_url() -> String {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("falta DATABASE_URL (necesita PostgreSQL de pruebas)"));
    if let Err(e) = check_test_database(&url) {
        panic!("{e}");
    }
    url
}

/// Un id propio de cada test (#688), por ejemplo para su instrumento: `<prefijo>-<proceso>-<n>`.
///
/// La hora sola no alcanza: los tests de un binario corren en paralelo, dos pueden leer el mismo
/// valor del reloj y entonces comparten el instrumento y ven las velas del otro. Acá `n` sale de
/// un reloj de microsegundos que **nunca se repite** dentro del proceso (si dos lecturas
/// coinciden, la segunda toma el valor siguiente), y el número de proceso separa dos corridas
/// simultáneas sobre la misma base. Con un prefijo de hasta 6 caracteres y un sufijo de fuente
/// como `.IBKR`, entra en un `InstrumentId` (32 caracteres).
pub fn unique_id(prefix: &str) -> String {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX));
    let next = |prev: u64| now.max(prev.saturating_add(1));
    let n = match LAST.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |p| Some(next(p))) {
        Ok(prev) | Err(prev) => next(prev),
    };
    // Los últimos 12 dígitos (11 días y medio): ningún proceso de tests dura tanto.
    format!("{prefix}-{}-{}", std::process::id(), n % 1_000_000_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #688: ids generados a la vez desde muchos hilos no se repiten nunca, aunque el reloj dé
    /// el mismo valor, y entran en un `InstrumentId` con el sufijo más largo de las fuentes.
    #[test]
    fn unique_ids_never_repeat_across_threads() {
        let threads: Vec<_> = (0..8)
            .map(|_| {
                std::thread::spawn(|| (0..2_000).map(|_| unique_id("ITGW")).collect::<Vec<_>>())
            })
            .collect();
        let ids: Vec<String> = threads
            .into_iter()
            .flat_map(|t| t.join().unwrap())
            .collect();
        let distinct: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(distinct.len(), ids.len(), "ids repetidos");
        for id in &ids {
            let derived: cfds_core::InstrumentId = format!("{id}.IBKR").parse().unwrap();
            assert!(derived.as_str().starts_with("ITGW-"));
        }
        let longest: cfds_core::InstrumentId =
            format!("{}.IBKR", unique_id("ITGWXY")).parse().unwrap();
        assert!(longest.as_str().len() <= 32);
    }

    #[test]
    fn reads_the_database_name() {
        assert_eq!(
            database_name("postgres://u:p@127.0.0.1:5432/cfds_test?sslmode=disable"),
            Some("cfds_test")
        );
        assert_eq!(
            database_name("postgres://u@h/cfdstrader"),
            Some("cfdstrader")
        );
        assert_eq!(database_name("postgres://u@h:5432"), None);
        assert_eq!(database_name("postgres://u@h:5432/"), None);
        assert_eq!(database_name("sin-esquema"), None);
    }

    #[test]
    fn only_test_databases_pass() {
        for ok in [
            "postgres://cfds:x@127.0.0.1:5432/cfds_test",
            "postgres://cfds:x@127.0.0.1:5432/cfds_test_guard",
            "postgresql://cfds@h/cfds_test_ml?sslmode=disable",
        ] {
            assert!(check_test_database(ok).is_ok(), "{ok}");
        }
        for bad in [
            "postgres://cfds:x@127.0.0.1:5432/cfdstrader",
            "postgres://cfds:x@127.0.0.1:5432/cfdstrader_test",
            "postgres://cfds:x@127.0.0.1:5432/postgres",
            "postgres://cfds:x@127.0.0.1:5432",
        ] {
            let e = check_test_database(bad).unwrap_err();
            assert!(e.contains("cfds_test"), "{bad}: {e}");
        }
        assert!(
            check_test_database("postgres://u@h/cfdstrader")
                .unwrap_err()
                .starts_with("los tests no escriben en la base real")
        );
    }

    #[test]
    fn swaps_the_database_keeping_params() {
        assert_eq!(
            with_database(
                "postgres://u:p@h:5432/cfds_test?sslmode=disable",
                "cfds_test_x"
            ),
            "postgres://u:p@h:5432/cfds_test_x?sslmode=disable"
        );
        assert_eq!(
            with_database("postgres://u:p@h:5432/cfds_test", "cfds_test_x"),
            "postgres://u:p@h:5432/cfds_test_x"
        );
    }
}
