"""Tests for the client embedder (embed.py): the real-model wrapper.

There is only one embedder in production -- see embed.py's module docstring."""

from __future__ import annotations

import math
import os

import pytest

from lloom import embed as embed_mod
from lloom.embed import Embedder

_REAL_MODEL_ID = "nomic-ai/nomic-embed-text-v1.5"


class _StubRow:
    """Mimics a 1-D ndarray row: exposes .tolist()."""

    def __init__(self, values: list[float]):
        self._values = values

    def tolist(self) -> list[float]:
        return list(self._values)


class _StubModel:
    """Shape-faithful sentence-transformers stand-in.

    encode() must receive a LIST of strings and returns a 2-D array-like (each
    row carries .tolist()). A bare str (the old bug) fails the batch
    assertion, so the old code cannot pass this test.
    """

    def __init__(self, dim: int = 4):
        self.dim = dim
        self.batches: list[list[str]] = []

    def encode(self, texts, **kwargs):
        assert isinstance(texts, list), "encode() must receive a batch (list of str)"
        self.batches.append(list(texts))
        return [_StubRow([(i + 1) / (len(t) + 1) for i in range(self.dim)]) for t in texts]


def test_embedder_passes_batch_list_to_model():
    stub = _StubModel(dim=4)
    embed_mod._MODEL_CACHE["stub://test"] = stub
    vec = Embedder("stub://test", backend_name="sentence-transformers").embed("hello")
    assert stub.batches == [["hello"]]
    assert len(vec) == 4
    assert all(isinstance(v, float) for v in vec)


def test_model_cached_per_process():
    stub = _StubModel(dim=3)
    embed_mod._MODEL_CACHE["stub://shared"] = stub
    assert Embedder("stub://shared", backend_name="sentence-transformers").embed("a") is not None
    assert Embedder("stub://shared", backend_name="sentence-transformers").embed("b") is not None
    assert stub.batches == [["a"], ["b"]]
    assert embed_mod.load_model("stub://shared") is stub


def _hf_model_cached(model_id: str) -> bool:
    cache = os.environ.get("HF_HOME", os.path.expanduser("~/.cache/huggingface"))
    hub = os.path.join(cache, "hub")
    if not os.path.isdir(hub):
        return False
    prefix = "models--" + model_id.replace("/", "--")
    return any(d.startswith(prefix) for d in os.listdir(hub))


@pytest.mark.real_embedder
def test_real_model_related_beats_unrelated():
    # A bare install is torch-free on purpose; local embedding is the
    # `local-embed` extra. Without it there is no real model to test.
    pytest.importorskip("sentence_transformers", reason="install the local-embed extra")
    if not _hf_model_cached(_REAL_MODEL_ID):
        pytest.skip(f"{_REAL_MODEL_ID} not in the local HF cache; skipping real-model test")
    emb = Embedder(backend_name="sentence-transformers")

    def dot(a, b) -> float:
        return sum(x * y for x, y in zip(a, b))

    related = emb.embed("sailing the mediterranean sea")
    near = emb.embed("yachts and sailboats")
    far = emb.embed("python debugging")
    assert len(related) == 768
    assert abs(math.sqrt(sum(v * v for v in related)) - 1.0) < 1e-3
    assert dot(related, near) > dot(related, far)


# -- the ollama backend -------------------------------------------------------
#
# A second BACKEND is not a second embedder: it is the one embedder a whole
# deployment (server + every client) is pointed at. These tests pin the wire
# contract and the switch, never a fallback between the two.


class _FakeResponse:
    def __init__(self, payload: dict, status: int = 200):
        self._payload = payload
        self.status_code = status

    def raise_for_status(self) -> None:
        if self.status_code >= 400:
            raise RuntimeError(f"HTTP {self.status_code}")

    def json(self) -> dict:
        return self._payload


def _fake_post(captured: dict, payload: dict, status: int = 200):
    def post(url, json=None, timeout=None):
        captured["url"] = url
        captured["json"] = json
        captured["timeout"] = timeout
        return _FakeResponse(payload, status)

    return post


def test_backend_defaults_to_server(monkeypatch):
    """The default placement is the hub: no local embedding, no torch."""
    monkeypatch.delenv("LLOOM_EMBED_BACKEND", raising=False)
    assert embed_mod.backend() == "server"
    assert Embedder().embed_or_none("hello") is None


def test_server_backend_never_imports_sentence_transformers(monkeypatch):
    import sys

    monkeypatch.delenv("LLOOM_EMBED_BACKEND", raising=False)
    sentinel = object()
    monkeypatch.setitem(sys.modules, "sentence_transformers", sentinel)
    e = Embedder()
    assert e.embed_or_none("hello") is None  # server-side: no import, no vector
    with pytest.raises(RuntimeError, match="hub"):
        e.embed("hello")  # explicit local embed is a misconfiguration
    assert sys.modules["sentence_transformers"] is sentinel  # untouched


def test_local_alias_selects_the_sentence_transformers_backend(monkeypatch):
    monkeypatch.setenv("LLOOM_EMBED_BACKEND", "local")
    assert embed_mod.backend() == "sentence-transformers"
    assert Embedder().model_id == _REAL_MODEL_ID
    monkeypatch.setenv("LLOOM_EMBED_BACKEND", "sentence-transformers")
    assert embed_mod.backend() == "sentence-transformers"


def test_backend_env_selects_ollama_and_its_default_model(monkeypatch):
    monkeypatch.setenv("LLOOM_EMBED_BACKEND", "ollama")
    monkeypatch.delenv("LLOOM_EMBED_MODEL", raising=False)
    e = Embedder()
    assert e.backend == "ollama"
    assert e.model_id == "nomic-embed-text"


def test_ollama_posts_to_the_standard_endpoint(monkeypatch):
    import httpx

    captured: dict = {}
    monkeypatch.setattr(httpx, "post", _fake_post(captured, {"embeddings": [[3.0, 4.0]]}))
    monkeypatch.setenv("LLOOM_EMBED_BACKEND", "ollama")
    monkeypatch.setenv("LLOOM_EMBED_MODEL", "nomic-embed-text")
    monkeypatch.delenv("LLOOM_OLLAMA_URL", raising=False)

    vec = Embedder().embed("a flat in Raval")
    assert captured["url"] == "http://localhost:11434/api/embed"
    assert captured["json"] == {"model": "nomic-embed-text", "input": "a flat in Raval"}
    # normalized: the profile says l2 and ollama does not normalize
    assert vec == [0.6, 0.8]
    assert math.isclose(math.sqrt(sum(v * v for v in vec)), 1.0)


def test_ollama_url_env_overrides_and_strips_a_trailing_slash(monkeypatch):
    import httpx

    captured: dict = {}
    monkeypatch.setattr(httpx, "post", _fake_post(captured, {"embeddings": [[1.0, 0.0]]}))
    monkeypatch.setenv("LLOOM_EMBED_BACKEND", "ollama")
    monkeypatch.setenv("LLOOM_OLLAMA_URL", "http://ollama.internal:11434/")
    Embedder().embed("x")
    assert captured["url"] == "http://ollama.internal:11434/api/embed"


def test_ollama_accepts_the_legacy_embedding_key(monkeypatch):
    """`/api/embeddings` (singular) answers {"embedding": [...]}, and costs
    nothing to keep working."""
    import httpx

    monkeypatch.setattr(httpx, "post", _fake_post({}, {"embedding": [0.0, 5.0]}))
    monkeypatch.setenv("LLOOM_EMBED_BACKEND", "ollama")
    assert Embedder().embed("x") == [0.0, 1.0]


def test_ollama_empty_response_raises_and_embed_or_none_swallows_it(monkeypatch):
    import httpx

    monkeypatch.setattr(httpx, "post", _fake_post({}, {"embeddings": []}))
    monkeypatch.setenv("LLOOM_EMBED_BACKEND", "ollama")
    with pytest.raises(RuntimeError, match="no embedding"):
        Embedder().embed("x")
    # the caller contract is unchanged: None means "send text, let the server embed"
    assert Embedder().embed_or_none("x") is None


def test_ollama_http_failure_is_none_not_a_substitute_vector(monkeypatch):
    import httpx

    monkeypatch.setattr(httpx, "post", _fake_post({}, {}, status=500))
    monkeypatch.setenv("LLOOM_EMBED_BACKEND", "ollama")
    assert Embedder().embed_or_none("x") is None


def test_ollama_zero_norm_is_an_error_not_a_zero_vector(monkeypatch):
    """A zero vector would be accepted by nothing downstream (the server
    rejects zero-norm) and matches everything at cosine 0 — fail instead."""
    import httpx

    monkeypatch.setattr(httpx, "post", _fake_post({}, {"embeddings": [[0.0, 0.0]]}))
    monkeypatch.setenv("LLOOM_EMBED_BACKEND", "ollama")
    with pytest.raises(RuntimeError, match="zero norm"):
        Embedder().embed("x")


def test_explicit_backend_argument_beats_the_env(monkeypatch):
    monkeypatch.setenv("LLOOM_EMBED_BACKEND", "ollama")
    assert Embedder(backend_name="sentence-transformers").model_id == _REAL_MODEL_ID

