---
modulo: intelligence
ruta: `intelligence/` (paquete `cfds_intel`)
lenguaje: Python
estado: hecho
resumen: Proyecto base de la capa de inteligencia y única salida HTTPS (`cfds_intel/net.py`).
---

# intelligence

## Responsabilidad (única)

- Proyecto base de la capa de inteligencia. `cfds_intel/net.py`: única salida HTTPS (noticias, COT), con el almacén de certificados del sistema (`truststore`) y sin desactivar nunca la verificación

## Estado

- esqueleto (#3), HTTPS con el almacén del sistema (#121)
