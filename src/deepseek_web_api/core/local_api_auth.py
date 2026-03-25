"""Local API authentication for protecting this proxy service."""

from fastapi import HTTPException, Request, status

from .config import get_local_api_key


def _extract_request_token(request: Request) -> str:
    """Extract API token from Authorization or X-API-Key headers."""
    x_api_key = request.headers.get("x-api-key", "").strip()
    if x_api_key:
        return x_api_key

    authorization = request.headers.get("authorization", "").strip()
    if authorization.lower().startswith("bearer "):
        return authorization[7:].strip()

    return ""


def requires_local_api_auth(path: str) -> bool:
    """Return True when the request path should be protected."""
    return path.startswith("/v0") or path.startswith("/v1")


def verify_local_api_auth(request: Request) -> None:
    """Validate local API auth when an API key is configured."""
    expected_token = get_local_api_key()
    if not expected_token:
        return

    provided_token = _extract_request_token(request)
    if provided_token == expected_token:
        return

    raise HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail="Invalid or missing local API key",
        headers={"WWW-Authenticate": "Bearer"},
    )
