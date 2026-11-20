# Respuestas grabadas de la API demo de IG (#66)

Los tests no llaman a la API: usan estas respuestas.

| Archivo | Pedido | Origen |
|---|---|---|
| `prices_de40_15m.json` | `GET /prices/IX.D.DAX.IFE.IP` v3, `MINUTE_15`, `from=2026-11-20T10:00:00`, `to=2026-11-20T11:00:00` (hora local de la cuenta) | Respuesta real del 2026-11-20, sin cambios |
| `prices_us100_1h_p{1,2,3}.json` | `GET /prices/IX.D.NASDAQ.IFE.IP` v3, `HOUR`, `pageSize=2`, páginas 1 a 3 | Respuestas reales del 2026-11-20, sin cambios |
| `session_v2.json` | `POST /session` v2 | **Inventado**, con la misma forma que la respuesta real: ids, saldos y endpoint son falsos |

Las respuestas de precios solo tienen datos de mercado (precios, cupo y paginado).
Nunca se graban tokens (`CST`, `X-SECURITY-TOKEN`), ids de cuenta reales ni credenciales.
