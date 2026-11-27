---
modulo: patterns
ruta: `engine/crates/patterns`
lenguaje: Rust
estado: hecho
resumen: Patrones de velas y chartistas, y herramientas de análisis que no entran al puntaje (Fibonacci y otras).
---

# patterns

## Responsabilidad (única)

- Patrones de velas (martillo, envolvente...) y chartistas (doble techo/suelo, soportes/resistencias)
- herramientas de análisis que no entran al puntaje: tramos de Fibonacci entre pivotes confirmados con retrocesos, extensiones y golden pocket, con φ exacto en `Decimal` (`fibonacci`, `config/analysis.toml`)
- spring y upthrust de Wyckoff sobre los soportes y resistencias de `levels` (`wyckoff`) y "precio de fondo": pivote bajo confirmado tras una bajada, con rebote (`bottom`)

## Estado

- pivotes y patrones de velas (#39), doble techo/suelo y soportes/resistencias (#41), Fibonacci, golden pocket, spring/upthrust y fondo (#491)
