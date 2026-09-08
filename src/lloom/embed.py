"""Embedding placement: the hub embeds by default; local is an opt-in.

Embedding runs ENTIRELY ON THE SERVER by default: a client without an
explicit backend sends text and lets the hub embed it (the send/agent
endpoints accept a message with no vector and embed it server-side). A bare
`pip install lloom-client` therefore carries no torch and no model weights.

`LLOOM_EMBED_BACKEND` opts a client back INTO embedding locally, for
deployments that pin it:

* `server` (default) -- no local embedding at all: no torch import, no
  weights, nothing to load. `embed_or_none` returns None and the caller sends
  text; the hub embeds it in the deployment's one embedding space.
* `local` (alias: `sentence-transformers`) -- the local nomic weights, as the
  client always did. Requires the `local-embed` extra
  (`pip install 'lloom-client[local-embed]'`); without it the load raises and
  `embed_or_none` degrades to None (hub embeds).
* `ollama` -- the standard Ollama endpoint (`LLOOM_OLLAMA_URL`, default
  `http://localhost:11434`), model `LLOOM_EMBED_MODEL`. Nothing to download
  into the process and no torch import, which is what makes the integration
  suite able to embed for real instead of skipping.

Whatever a client embeds with MUST be the deployment's one embedder -- the
same backend and model the hub itself runs. A second embedder can only
produce vectors in a different space, and cosine similarity across the two is
noise rather than an error anything can detect -- an agent embedded with a
stand-in silently stops matching. This is a choice made once for a server and
every client that talks to it -- NOT a fallback and never a per-call
decision. The model load happens lazily on first use and is cached per
process, keyed by model id (shared across Embedder instances). When a local
model cannot be loaded, `embed_or_none` returns None and the caller sends
text (the hub embeds it) instead of substituting a vector.
"""

from __future__ import annotations

import math
import os
from typing import Any

_MODEL_CACHE: dict[str, Any] = {}

DEFAULT_MODEL_ID = "nomic-ai/nomic-embed-text-v1.5"
DEFAULT_OLLAMA_URL = "http://localhost:11434"
#: read at call time, not import time, so a test can set it per subprocess
BACKEND_ENV = "LLOOM_EMBED_BACKEND"
MODEL_ENV = "LLOOM_EMBED_MODEL"
OLLAMA_URL_ENV = "LLOOM_OLLAMA_URL"

#: the default is NO local embedding: the hub embeds server-side
SERVER_BACKEND = "server"
_LOCAL_ALIASES = ("local", "sentence-transformers")


def backend() -> str:
    """The client's embedding placement: `server` (default), `local`, or
    `ollama`."""
    raw = (os.environ.get(BACKEND_ENV) or SERVER_BACKEND).strip().lower()
    return "sentence-transformers" if raw in _LOCAL_ALIASES else raw


def load_model(model_id: str) -> Any:
    """Load the model once per process and cache it; lazy on first use."""
    model = _MODEL_CACHE.get(model_id)
    if model is None:
        try:
            from sentence_transformers import SentenceTransformer
        except ImportError as exc:
            raise RuntimeError(
                "sentence-transformers is not installed; install the"
                " local-embed extra (`pip install 'lloom-client[local-embed]'`)"
                " to embed locally"
            ) from exc
        # CPU-only: embedding batches are tiny and GPU kernels abort on some hosts
        model = SentenceTransformer(model_id, device="cpu")
        _MODEL_CACHE[model_id] = model
    return model


def _l2(vec: list[float]) -> list[float]:
    """Normalize to unit length -- the profile is `l2` and Ollama does not
    normalize, while sentence-transformers is asked to."""
    norm = math.sqrt(sum(v * v for v in vec))
    if norm == 0:
        raise RuntimeError("embedding has zero norm")
    return [float(v / norm) for v in vec]


def embed_via_ollama(text: str, model_id: str, url: str, timeout: float = 60.0) -> list[float]:
    """One embedding from the standard Ollama endpoint (`POST /api/embed`)."""
    import httpx

    r = httpx.post(
        f"{url.rstrip('/')}/api/embed",
        json={"model": model_id, "input": text},
        timeout=timeout,
    )
    r.raise_for_status()
    body = r.json()
    # /api/embed answers {"embeddings": [[...]]}; the older /api/embeddings
    # spelling answers {"embedding": [...]} and costs nothing to accept
    vecs = body.get("embeddings")
    vec = vecs[0] if vecs else body.get("embedding")
    if not vec:
        raise RuntimeError(f"ollama returned no embedding for model {model_id}")
    return _l2([float(v) for v in vec])


class Embedder:
    """The one embedder this deployment uses, per this client's placement."""

    def __init__(self, model_id: str | None = None, backend_name: str | None = None):
        self.backend = (backend_name or backend()).strip().lower()
        if model_id is not None:
            self.model_id = model_id
        elif self.backend == "ollama":
            self.model_id = os.environ.get(MODEL_ENV) or "nomic-embed-text"
        else:
            self.model_id = os.environ.get(MODEL_ENV) or DEFAULT_MODEL_ID

    def embed(self, text: str) -> list[float]:
        if self.backend == SERVER_BACKEND:
            raise RuntimeError(
                "embedding runs on the hub; to embed locally install the"
                " local-embed extra (`pip install 'lloom-client[local-embed]'`)"
                " and set LLOOM_EMBED_BACKEND=local"
            )
        if self.backend == "ollama":
            url = os.environ.get(OLLAMA_URL_ENV) or DEFAULT_OLLAMA_URL
            return embed_via_ollama(text, self.model_id, url)
        model = load_model(self.model_id)
        vec = model.encode([text], normalize_embeddings=True)[0].tolist()
        return [float(v) for v in vec]

    def embed_or_none(self, text: str) -> list[float] | None:
        """A vector, or None meaning "send text; the hub embeds it".

        The default (`server` placement) always answers None: embedding is
        not a client operation unless it was opted into.
        """
        if self.backend == SERVER_BACKEND:
            return None
        try:
            return self.embed(text)
        except Exception:
            return None
