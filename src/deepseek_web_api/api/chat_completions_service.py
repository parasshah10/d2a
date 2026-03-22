"""Chat completions service - extracted business logic from routes.py."""

import asyncio
import json
import logging

import httpx
from fastapi import Response

from ..core.auth import get_auth_headers
from ..core.pow_service import get_pow_response
from ..core.session_store import SessionStore
from ..core.config import DEEPSEEK_HOST

logger = logging.getLogger("deepseek_web_api")

# API path constants
_PATH_CREATE_SESSION = "api/v0/chat_session/create"
_PATH_DELETE_SESSION = "api/v0/chat_session/delete"
_PATH_COMPLETION = "api/v0/chat/completion"
_PATH_UPLOAD_FILE = "api/v0/file/upload_file"
_PATH_FETCH_FILES = "api/v0/file/fetch_files"
_PATH_HISTORY_MESSAGES = "api/v0/chat/history_messages"

DEEPSEEK_BASE_URL = f"https://{DEEPSEEK_HOST}"
session_store = SessionStore.get_instance()


def parse_sse_response_message_id(content: bytes) -> int | None:
    """Parse SSE stream to extract response_message_id from ready event."""
    try:
        text = content.decode("utf-8")
        for line in text.split("\n"):
            line = line.strip()
            if line.startswith("data:") and "response_message_id" in line:
                data_str = line[5:].strip()
                data = json.loads(data_str)
                return data.get("response_message_id")
    except Exception as e:
        logger.warning(f"Failed to parse SSE response_message_id: {type(e).__name__}")
    return None


def extract_chat_session_id(resp_body: bytes) -> str | None:
    """Extract chat_session_id from DeepSeek API response."""
    try:
        data = json.loads(resp_body)
        return data.get("data", {}).get("biz_data", {}).get("id")
    except Exception as e:
        logger.warning(f"Failed to extract chat_session_id: {type(e).__name__}")
        return None


async def proxy_to_deepseek(
    method,
    path,
    headers=None,
    json_data=None,
    params=None,
    content=None,
    files=None,
):
    """Proxy request to DeepSeek backend, return FastAPI Response."""
    url = f"{DEEPSEEK_BASE_URL}/{path}"
    auth_headers = get_auth_headers()
    if headers:
        headers = {**headers, **auth_headers}
    else:
        headers = auth_headers
    headers["Host"] = DEEPSEEK_HOST

    if files is not None and "Content-Type" in headers:
        del headers["Content-Type"]

    async with httpx.AsyncClient(timeout=120.0) as client:
        resp = await client.request(
            method=method,
            url=url,
            headers=headers,
            json=json_data,
            params=params,
            content=content,
            files=files,
        )
        return Response(
            content=resp.content,
            status_code=resp.status_code,
            headers=dict(resp.headers),
        )


async def proxy_to_deepseek_stream(
    method,
    path,
    headers=None,
    json_data=None,
    params=None,
):
    """Proxy request to DeepSeek backend as a streaming response, yield bytes."""
    url = f"{DEEPSEEK_BASE_URL}/{path}"
    auth_headers = get_auth_headers()
    if headers:
        headers = {**headers, **auth_headers}
    else:
        headers = auth_headers
    headers["Host"] = DEEPSEEK_HOST

    async with httpx.AsyncClient(timeout=120.0) as client:
        async with client.stream(
            method=method,
            url=url,
            headers=headers,
            json=json_data,
            params=params,
        ) as resp:
            async for chunk in resp.aiter_bytes():
                yield chunk


async def create_session_on_deepseek() -> str | None:
    """Create session on DeepSeek backend and return the chat_session_id."""
    resp = await proxy_to_deepseek(
        "POST",
        _PATH_CREATE_SESSION,
        json_data={"agent": "chat"},
    )
    # resp is FastAPI Response, access body via .body
    if resp.body:
        try:
            csid = extract_chat_session_id(resp.body)
            if csid:
                await session_store.acreate_session(csid)
                return csid
        except Exception as e:
            logger.warning(f"Failed to create session: {e}")
    return None


async def delete_session(chat_session_id: str) -> None:
    """Delete session from DeepSeek backend and clean up local store."""
    max_retries = 5
    retry_delay = 0.5
    last_exc = None

    for attempt in range(max_retries):
        try:
            resp = await proxy_to_deepseek(
                "POST",
                _PATH_DELETE_SESSION,
                json_data={"chat_session_id": chat_session_id},
            )
            logger.warning(f"[delete_session] attempt {attempt+1}: status={resp.status_code}")

            # Check biz_code from delete response
            if resp.body:
                try:
                    data = json.loads(resp.body)
                    biz_code = data.get("data", {}).get("biz_code")
                    if biz_code == 0:
                        # Delete succeeded
                        logger.warning("[delete_session] session deleted")
                        break
                    else:
                        biz_msg = data.get("data", {}).get("biz_msg", "unknown error")
                        logger.warning(f"[delete_session] biz_code={biz_code}, msg={biz_msg}, retrying...")
                except json.JSONDecodeError:
                    logger.warning(f"[delete_session] failed to parse response, retrying...")

            # Retry on failure
            if attempt < max_retries - 1:
                await asyncio.sleep(retry_delay)
                retry_delay *= 2
            continue
        except Exception as e:
            last_exc = e
            logger.warning(f"[delete_session] attempt {attempt+1} exception: {type(e).__name__}: {e}")
            if attempt < max_retries - 1:
                await asyncio.sleep(retry_delay)
                retry_delay *= 2
            continue

    if last_exc:
        logger.warning(f"[delete_session] all {max_retries} attempts failed, last error: {last_exc}")

    # Always clean up local store, regardless of backend result
    await session_store.adelete_session(chat_session_id)


async def create_session(body: dict = None) -> Response:
    """Create new session and return response with chat_session_id added.

    Args:
        body: Request body, defaults to {"agent": "chat"}

    Returns:
        FastAPI Response with chat_session_id added to body
    """
    if body is None:
        body = {"agent": "chat"}

    resp = await proxy_to_deepseek(
        "POST",
        _PATH_CREATE_SESSION,
        json_data=body,
    )

    if resp.body:
        try:
            chat_session_id = extract_chat_session_id(resp.body)
            if chat_session_id:
                await session_store.acreate_session(chat_session_id)
                data = json.loads(resp.body)
                data["chat_session_id"] = chat_session_id
                return Response(
                    content=json.dumps(data),
                    status_code=resp.status_code,
                    headers={"Content-Type": "application/json"},
                )
        except Exception as e:
            logger.warning(f"Failed to process session response: {e}")

    return Response(
        content=resp.body,
        status_code=resp.status_code,
        headers={"Content-Type": "application/json"},
    )


async def upload_file(file_content: bytes, filename: str, content_type: str) -> Response:
    """Upload file to DeepSeek.

    Args:
        file_content: File binary content
        filename: File name
        content_type: MIME type

    Returns:
        FastAPI Response from DeepSeek
    """
    files = {"file": (filename, file_content, content_type)}
    pow_response = get_pow_response(target_path="/api/v0/file/upload_file")
    headers = {
        "x-ds-pow-response": pow_response,
        "x-file-size": str(len(file_content)),
    } if pow_response else {}

    return await proxy_to_deepseek(
        "POST",
        _PATH_UPLOAD_FILE,
        headers=headers,
        files=files,
    )


async def fetch_files(file_ids: str) -> Response:
    """Fetch file status from DeepSeek.

    Args:
        file_ids: Comma-separated file IDs

    Returns:
        FastAPI Response from DeepSeek
    """
    return await proxy_to_deepseek(
        "GET",
        _PATH_FETCH_FILES,
        params={"file_ids": file_ids},
    )


async def get_history_messages(chat_session_id: str, offset: int = 0, limit: int = 20) -> Response:
    """Get chat history from DeepSeek.

    Args:
        chat_session_id: Session ID
        offset: Message offset
        limit: Message limit

    Returns:
        FastAPI Response from DeepSeek
    """
    return await proxy_to_deepseek(
        "GET",
        _PATH_HISTORY_MESSAGES,
        params={"chat_session_id": chat_session_id, "offset": offset, "limit": limit},
    )


async def stream_chat_completion(
    prompt: str,
    chat_session_id: str | None = None,
    search_enabled: bool = True,
    thinking_enabled: bool = True,
    ref_file_ids: list | None = None,
):
    """Stream chat completion from DeepSeek and yield SSE bytes.

    This is an async generator that yields raw SSE bytes chunks.
    Session management is handled internally:
    - If chat_session_id is provided, it will be used (multi-turn conversation)
    - If not provided, a new session is created and cleaned up after

    Args:
        prompt: The prompt to send
        chat_session_id: Optional existing session ID for multi-turn
        search_enabled: Enable web search
        thinking_enabled: Enable thinking/reasoning
        ref_file_ids: Optional list of file IDs to reference

    Yields:
        bytes: Raw SSE response chunks from DeepSeek
    """
    # Determine chat_session_id and parent_message_id
    if chat_session_id:
        parent_message_id = await session_store.aget_parent_message_id(chat_session_id)
        if not await session_store.ahas_session(chat_session_id):
            await session_store.acreate_session(chat_session_id)
            parent_message_id = None
    else:
        # Pre-create session so we can return the session_id in header
        chat_session_id = await create_session_on_deepseek()
        parent_message_id = None

    # Get PoW
    pow_response = get_pow_response()

    # Build payload for DeepSeek
    payload = {
        "chat_session_id": chat_session_id,
        "parent_message_id": parent_message_id,
        "preempt": False,
        "prompt": prompt,
        "ref_file_ids": ref_file_ids or [],
        "search_enabled": search_enabled,
        "thinking_enabled": thinking_enabled,
    }

    headers = {"x-ds-pow-response": pow_response} if pow_response else {}

    # Stream and collect
    collected = b""
    async for chunk in proxy_to_deepseek_stream(
        "POST",
        _PATH_COMPLETION,
        headers=headers,
        json_data=payload,
    ):
        collected += chunk
        yield chunk

    # Update session with message_id after stream completes
    if chat_session_id:
        msg_id = parse_sse_response_message_id(collected)
        if msg_id:
            await session_store.aupdate_parent_message_id(chat_session_id, msg_id)
