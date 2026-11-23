//! Las migraciones metidas en el binario son las de `infra/migrations/` (#348): `build.rs` le
//! dice a cargo que recompile cuando cambia esa carpeta, con la misma ruta que `migrate!`.
//! Sin base de datos.

use std::path::{Path, PathBuf};

use cfds_storage::migrator;

const DIR: &str = "../../../infra/migrations";

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sql_files() -> Vec<i64> {
    let mut v: Vec<i64> = std::fs::read_dir(crate_dir().join(DIR))
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            let (num, _) = name.strip_suffix(".sql")?.split_once('_')?;
            num.parse().ok()
        })
        .collect();
    v.sort();
    v
}

#[test]
fn the_build_script_watches_the_same_folder_as_migrate() {
    let build = std::fs::read_to_string(crate_dir().join("build.rs")).unwrap();
    assert!(
        build.contains(&format!("cargo:rerun-if-changed={DIR}")),
        "build.rs tiene que mirar {DIR}"
    );
    let pg = std::fs::read_to_string(crate_dir().join("src/pg.rs")).unwrap();
    assert!(pg.contains(&format!("sqlx::migrate!(\"{DIR}\")")));
    let dir = crate_dir().join(DIR);
    assert!(Path::is_dir(&dir), "{} no existe", dir.display());
    assert!(dir.ends_with("infra/migrations"));
}

#[test]
fn every_migration_file_is_embedded() {
    let files = sql_files();
    assert!(!files.is_empty());
    let embedded: Vec<i64> = migrator().migrations.iter().map(|m| m.version).collect();
    assert_eq!(
        embedded, files,
        "las migraciones del binario no son las de la carpeta: hay que recompilar"
    );
}
