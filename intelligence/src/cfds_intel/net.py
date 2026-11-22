"""Única salida HTTPS de `intelligence/` (#121).

Verifica los certificados con el **almacén del sistema operativo** (paquete `truststore`): el
llavero en macOS, el almacén de Windows y el de OpenSSL en Linux. Así los feeds del BCE andan en
macOS sin exportar raíces ni apuntar `SSL_CERT_FILE`.

Reglas:
- La verificación nunca se desactiva: certificado y nombre del servidor, siempre.
- No se toca el `ssl` global (`truststore.inject_into_ssl`): el contexto se pasa explícito en
  cada pedido, para que nada fuera de este módulo cambie de comportamiento.
- Solo `https://`: cualquier otro esquema es un error antes de abrir la conexión.
"""

import ssl
import urllib.request
from functools import cache

import truststore


class InsecureUrlError(ValueError):
    """Se pidió una URL que no es `https://`: no se abre ninguna conexión."""


@cache
def tls_context() -> ssl.SSLContext:
    """Contexto TLS con el almacén del sistema, verificación de certificado y de nombre."""
    ctx = truststore.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    ctx.minimum_version = ssl.TLSVersion.TLSv1_2
    return ctx


def https_get(req: urllib.request.Request, timeout: int) -> bytes:
    """Baja el cuerpo de `req`. Es el `opener` por omisión de noticias y COT."""
    if not req.full_url.startswith("https://"):
        raise InsecureUrlError(f"solo https: {req.full_url!r}")
    with urllib.request.urlopen(req, timeout=timeout, context=tls_context()) as resp:  # noqa: S310 (solo https, validado arriba)
        body: bytes = resp.read()
        return body
