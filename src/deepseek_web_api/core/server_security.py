"""Server-side security warnings for local deployment."""

from .config import get_cors_origins, get_local_api_key, get_server_host
from .logger import logger


_LOOPBACK_HOSTS = {"127.0.0.1", "localhost", "::1"}


def is_loopback_host(host: str) -> bool:
    normalized = host.strip().lower().strip("[]")
    return normalized in _LOOPBACK_HOSTS


def collect_startup_security_warnings() -> list[str]:
    host = get_server_host()
    api_key = get_local_api_key()
    cors_origins = get_cors_origins()

    warnings = []
    if not api_key:
        warnings.append("Local API auth is disabled; /v0 and /v1 are usable by any caller that can reach this service.")

    if "*" in cors_origins:
        warnings.append("CORS allows all origins; narrow [server].cors_origins before exposing browser clients.")

    if not is_loopback_host(host):
        warnings.append(f"Server host is {host}, not loopback; this service may be reachable from other machines.")

    if not is_loopback_host(host) and not api_key:
        warnings.append("Non-loopback binding without local API auth is unsafe for shared networks or public hosts.")

    return warnings


def log_startup_security_warnings() -> None:
    for warning in collect_startup_security_warnings():
        logger.warning(f"[security] {warning}")
