//! `sqlx::migrate!` mete las migraciones de `infra/migrations/` en el binario al compilar. Cargo no
//! vuelve a compilar el crate si solo se agrega un archivo a esa carpeta, y entonces un binario
//! viejo no aplica las migraciones nuevas. Esta línea le avisa a cargo que mire la carpeta, como
//! recomienda la documentación de sqlx. Es la misma ruta que `migrate!` en `src/pg.rs`, relativa
//! a este crate.

fn main() {
    println!("cargo:rerun-if-changed=../../../infra/migrations");
}
