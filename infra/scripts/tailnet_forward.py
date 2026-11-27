"""Reenvía la terminal (Vite en 127.0.0.1:5173) SOLO a la IP de Tailscale de esta Mac (#859).

Escucha únicamente en la dirección 100.x de Tailscale, así que solo los aparatos de la red
privada del dueño pueden entrar. Nada queda abierto al wifi ni a internet.

Uso: python3 infra/scripts/tailnet_forward.py [--port 5173]
"""

import argparse
import asyncio
import subprocess

TAILSCALE = "/Applications/Tailscale.app/Contents/MacOS/Tailscale"
# Vite escucha en "localhost" (en macOS suele ser ::1, no 127.0.0.1).
TARGET = ("localhost", 5173)


def tailnet_ip() -> str:
    out = subprocess.run([TAILSCALE, "ip", "-4"], capture_output=True, text=True, check=True)
    ip = out.stdout.strip().splitlines()[0]
    if not ip.startswith("100."):
        raise SystemExit(f"IP de Tailscale inesperada: {ip!r}")
    return ip


async def pipe(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
    try:
        while data := await reader.read(65536):
            writer.write(data)
            await writer.drain()
    except (ConnectionError, asyncio.CancelledError):
        pass
    finally:
        writer.close()


async def handle(client_r: asyncio.StreamReader, client_w: asyncio.StreamWriter) -> None:
    try:
        up_r, up_w = await asyncio.open_connection(*TARGET)
    except OSError:
        client_w.close()
        return
    await asyncio.gather(pipe(client_r, up_w), pipe(up_r, client_w))


async def main(port: int) -> None:
    ip = tailnet_ip()
    server = await asyncio.start_server(handle, ip, port)
    print(f"terminal en http://{ip}:{port} (solo red de Tailscale)", flush=True)
    async with server:
        await server.serve_forever()


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=5173)
    asyncio.run(main(ap.parse_args().port))
