"""The shared GeoPoint: ranges, and the shape the wire expects.

The model exists so the MCP tool schema can TELL the model what a location is
(two named fields, decimal degrees, bounded) instead of taking an opaque dict.
Its validators are the last line before a hallucinated or swapped pair reaches
the server, so they are worth testing directly.
"""

from __future__ import annotations

import pytest
from pydantic import ValidationError

from lloom.geo import GeoPoint

# a few real centroids the skills teach agents to resolve to
BARCELONA = (41.3874, 2.1686)
RAVAL = (41.3797, 2.1686)
GRACIA = (41.4036, 2.1560)


@pytest.mark.parametrize(
    ("lat", "lng"),
    [BARCELONA, RAVAL, GRACIA, (0.0, 0.0), (-90.0, -180.0), (90.0, 180.0)],
)
def test_accepts_points_on_and_inside_the_bounds(lat, lng):
    p = GeoPoint(lat=lat, lng=lng)
    assert (p.lat, p.lng) == (lat, lng)


@pytest.mark.parametrize(
    ("lat", "lng"), [(90.1, 0.0), (-90.1, 0.0), (0.0, 180.1), (0.0, -180.1)]
)
def test_rejects_points_outside_the_bounds(lat, lng):
    with pytest.raises(ValidationError):
        GeoPoint(lat=lat, lng=lng)


def test_a_swapped_barcelona_pair_is_still_in_range():
    """Pins what the type system can and cannot catch: `2.1686,41.3874` is a
    swapped Barcelona, and both numbers are legal degrees, so it validates.
    Ordering is the SKILL's job ("latitude first"), not the model's — only a
    swap that lands out of range is caught here."""
    swapped = GeoPoint(lat=2.1686, lng=41.3874)
    assert (swapped.lat, swapped.lng) == (2.1686, 41.3874)
    with pytest.raises(ValidationError):
        GeoPoint(lat=100.0, lng=60.0)


def test_model_dump_is_the_wire_shape():
    """The proxy hands this dict to the payload; the maildir and the outbox
    digest both require a plain {lat, lng}."""
    assert GeoPoint(lat=41.3797, lng=2.1686).model_dump() == {"lat": 41.3797, "lng": 2.1686}


def test_requires_both_fields():
    for kwargs in ({"lat": 41.4}, {"lng": 2.15}, {}):
        with pytest.raises(ValidationError):
            GeoPoint(**kwargs)


def test_coerces_integer_degrees():
    """A model answering `{"lat": 41, "lng": 2}` is coarse, not wrong."""
    p = GeoPoint(lat=41, lng=2)
    assert (p.lat, p.lng) == (41.0, 2.0)


def test_schema_documents_the_units_for_the_model():
    """These descriptions are prompt surface in the MCP tool schema."""
    props = GeoPoint.model_json_schema()["properties"]
    assert "decimal degrees" in props["lat"]["description"]
    assert "decimal degrees" in props["lng"]["description"]
