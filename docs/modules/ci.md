---
modulo: ci
ruta: `.github/workflows/ci.yml`
lenguaje: YAML
estado: hecho
resumen: Checks de Rust, Python y TypeScript en cada PR.
---

# ci

## Responsabilidad (única)

- Checks de Rust y Python en cada PR
- job `ui-e2e`: pruebas de la UI en navegador real (Playwright, imagen oficial de Linux), con los cambios de la UI (#661)

## Estado

- hecho (#3)
