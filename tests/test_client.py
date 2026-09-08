"""Client lib tests using a lightweight httpx MockTransport."""

from __future__ import annotations

import asyncio

import pytest

from lloom.client import AsyncClient, Client, LloomError


class _Mock:
    def __init__(self, handler):
        import httpx

        self._transport = httpx.MockTransport(handler)

    def build(self) -> Client:
        c = Client("http://test", "llm_key")
        c._http._transport = self._transport
        return c

    def build_async(self) -> AsyncClient:
        c = AsyncClient("http://test", "llm_key")
        c._http._transport = self._transport
        return c


def _json_handler(status: int, payload: dict):
    import httpx

    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(status, json=payload)

    return handler


def test_register_sets_key():
    mock = _Mock(_json_handler(201, {"agent_id": "agent:1", "handle": "agent0", "api_key": "llm_new"}))
    with mock.build() as c:
        res = c.register("@agent0", "password123456")
    assert res["api_key"] == "llm_new"


def test_send_private_payload():
    import httpx

    def handler(request: httpx.Request) -> httpx.Response:
        body = request.read().decode()
        assert '"kind":"private"' in body
        assert '"to":"@other"' in body
        return httpx.Response(201, json={"message_id": "message:1", "recipient_count": 1})

    mock = _Mock(handler)
    with mock.build() as c:
        res = c.send_private("@other", "hi")
    assert res["message_id"] == "message:1"


def test_auth_header_sent():
    import httpx

    def handler(request: httpx.Request) -> httpx.Response:
        assert request.headers["Authorization"] == "Bearer llm_key"
        return httpx.Response(200, json={"agent_id": "agent:1", "handle": "a", "scopes": [], "status": "active"})

    mock = _Mock(handler)
    with mock.build() as c:
        c.whoami()


def test_error_envelope():
    mock = _Mock(
        _json_handler(422, {"error": {"code": "recipient_inactive", "message": "inactive", "details": {}}})
    )
    with mock.build() as c, pytest.raises(LloomError) as ei:
        c.send_private("@gone", "hi")
    assert ei.value.code == "recipient_inactive"
    assert ei.value.status == 422


def test_mailbox_poll_and_ack():
    import httpx

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path.endswith("/ack"):
            return httpx.Response(200, json={"delivery_id": "delivery:1", "state": "acked"})
        return httpx.Response(
            200,
            json={"deliveries": [{"delivery_id": "delivery:1", "message_id": "m:1", "sender": "a", "kind": "private", "body": "hi", "state": "read"}], "next_cursor": None},
        )

    mock = _Mock(handler)
    with mock.build() as c:
        mail = c.mailbox()
        assert mail["deliveries"][0]["body"] == "hi"
        ack = c.ack("delivery:1")
        assert ack["state"] == "acked"


def test_send_broadcast_geo_tags_payload():
    import httpx

    def handler(request: httpx.Request) -> httpx.Response:
        body = request.read().decode()
        assert '"kind":"broadcast"' in body
        assert '"tags"' in body and '"restaurant"' in body and '"buyer"' in body
        assert '"radius_km":50' in body
        return httpx.Response(201, json={"message_id": "message:1", "recipient_count": 1, "recipients": ["agent:1"]})

    mock = _Mock(handler)
    with mock.build() as c:
        res = c.send_broadcast(
            "local deal",
            tags=["restaurant", "buyer"],
            location={"lat": 40.7128, "lng": -74.006},
            radius_km=50,
        )
    assert res["message_id"] == "message:1"


# -- P8: AsyncClient mirrors the sync surface (same envelope handling) ---------


def test_async_client_surface_and_error_parity():
    import httpx

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == "/v1/auth/whoami":
            assert request.headers["Authorization"] == "Bearer llm_key"
            return httpx.Response(200, json={"agent_id": "agent:1", "handle": "a"})
        if request.url.path == "/v1/mailbox":
            return httpx.Response(200, json={"deliveries": [], "next_cursor": None})
        return httpx.Response(429, json={"error": {"code": "quota_exceeded", "message": "slow down"}}, headers={"Retry-After": "2"})

    mock = _Mock(handler)

    async def scenario() -> dict:
        async with mock.build_async() as c:
            me = await c.whoami()
            assert me["handle"] == "a"
            box = await c.mailbox(limit=5, wait=30)
            assert box["deliveries"] == []
            await c.register("@a", "password123456")  # 429 from the stub
            return {}

    with pytest.raises(LloomError) as ei:
        asyncio.run(scenario())
    assert ei.value.code == "quota_exceeded"
    assert ei.value.status == 429
    assert ei.value.retry_after == 2.0


# -- review F3: long-poll timeout must exceed the server's wait window -----------


def test_mailbox_longpoll_timeout_extends_past_wait_window():
    """F3 (review): the default 30 s timeout equals the server's
    longpoll_max_wait — an idle mailbox(wait=30) would client-timeout
    before the server's normal empty return. The request must carry a
    timeout of at least wait + 15 s (and the plain default otherwise)."""
    import httpx

    seen: list[dict] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request.extensions.get("timeout"))
        return httpx.Response(200, json={"deliveries": [], "next_cursor": None})

    mock = _Mock(handler)
    with mock.build() as c:
        c.mailbox(wait=30)
    assert seen[-1]["read"] >= 30 + 15.0  # wait window + headroom

    with mock.build() as c:
        c.mailbox()  # no wait: the client default applies unchanged
    assert seen[-1]["read"] == 30.0


def test_async_mailbox_longpoll_timeout_extends_past_wait_window():
    import httpx

    seen: list[dict] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request.extensions.get("timeout"))
        return httpx.Response(200, json={"deliveries": [], "next_cursor": None})

    mock = _Mock(handler)

    async def scenario():
        async with mock.build_async() as c:
            await c.mailbox(wait=30)
            await c.mailbox()

    asyncio.run(scenario())
    assert seen[0]["read"] >= 45.0
    assert seen[1]["read"] == 30.0

# -- handle collisions ------------------------------------------------------


_TAKEN = {
    "error": {
        "code": "handle_taken",
        "message": "handle marta_coll already taken",
        "details": {"suggestions": ["coll_marta", "m_coll", "marta_coll_2"]},
    }
}


def test_handle_taken_error_carries_suggestions():
    mock = _Mock(_json_handler(409, _TAKEN))
    with mock.build() as c, pytest.raises(LloomError) as excinfo:
        c.register("@marta_coll", "password123456")
    exc = excinfo.value
    assert exc.code == "handle_taken"
    assert exc.suggestions == ["coll_marta", "m_coll", "marta_coll_2"]
    assert exc.details["suggestions"] == exc.suggestions


def test_handle_taken_error_carries_suggestions_async():
    mock = _Mock(_json_handler(409, _TAKEN))

    async def go():
        c = mock.build_async()
        try:
            with pytest.raises(LloomError) as excinfo:
                await c.register("@marta_coll", "password123456")
            return excinfo.value
        finally:
            await c.aclose()

    assert asyncio.run(go()).suggestions == ["coll_marta", "m_coll", "marta_coll_2"]


def test_error_without_details_has_empty_suggestions():
    mock = _Mock(_json_handler(400, {"error": {"code": "invalid_request", "message": "bad"}}))
    with mock.build() as c, pytest.raises(LloomError) as excinfo:
        c.register("@ab", "password123456")
    assert excinfo.value.details == {}
    assert excinfo.value.suggestions == []


def test_check_handle_requests_the_handle_as_a_query_param():
    import httpx

    seen: dict = {}

    def handler(request: httpx.Request) -> httpx.Response:
        seen["url"] = str(request.url)
        seen["method"] = request.method
        return httpx.Response(
            200, json={"handle": "marta_coll", "available": False, "suggestions": ["coll_marta"]}
        )

    mock = _Mock(handler)
    with mock.build() as c:
        res = c.check_handle("@marta_coll")
    assert seen["method"] == "GET"
    assert seen["url"] == "http://test/v1/handles/check?handle=%40marta_coll"
    assert res["available"] is False
    assert res["suggestions"] == ["coll_marta"]


def test_board_methods_hit_the_board_routes():
    import httpx

    seen: list[tuple[str, str]] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append((request.method, request.url.path))
        if request.method == "POST" and request.url.path == "/v1/boards":
            return httpx.Response(201, json={"board_id": "board:1", "title": "ops", "owner": "agent:1"})
        if request.method == "POST" and request.url.path.endswith("/members"):
            return httpx.Response(201, json={"agent_id": "agent:2", "handle": "b", "changed": True})
        return httpx.Response(200, json={"boards": [], "messages": [], "changed": False, "members": []})

    mock = _Mock(handler)
    with mock.build() as c:
        assert c.board_create("ops")["board_id"] == "board:1"
        c.boards()
        c.board("board:1")
        c.board_messages("board:1")
        c.board_add_member("board:1", "@b")
        c.board_remove_member("board:1", "@b")
        c.board_leave("board:1")
    assert seen == [
        ("POST", "/v1/boards"),
        ("GET", "/v1/boards"),
        ("GET", "/v1/boards/board:1"),
        ("GET", "/v1/boards/board:1/messages"),
        ("POST", "/v1/boards/board:1/members"),
        ("DELETE", "/v1/boards/board:1/members/@b"),
        ("DELETE", "/v1/boards/board:1/membership"),
    ]


def test_send_board_payload_carries_kind_and_board():
    import httpx

    def handler(request: httpx.Request) -> httpx.Response:
        body = request.read().decode()
        assert '"kind":"board"' in body
        assert '"board":"board:abc"' in body
        assert '"to":null' in body
        return httpx.Response(201, json={"message_id": "message:9", "recipient_count": 2})

    mock = _Mock(handler)
    with mock.build() as c:
        res = c.send_board("board:abc", "hello board")
    assert res["recipient_count"] == 2


def test_async_board_surface_parity():
    import httpx

    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, json={"boards": [], "messages": [], "changed": False})

    mock = _Mock(handler)

    async def scenario() -> None:
        async with mock.build_async() as c:
            await c.board_create("t")
            await c.boards()
            await c.board_messages("board:1")
            await c.board_leave("board:1")

    asyncio.run(scenario())
