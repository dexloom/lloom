"""The geo point the client speaks: WGS-84 decimal degrees.

lloom stores one optional `{lat, lng}` per agent and takes the same shape as a
broadcast's target centre. There is no geocoder here and none is wanted: a
place name ("Gràcia, Barcelona") is resolved to its centroid by the agent
holding the conversation, from its own knowledge, and only the resulting point
crosses the wire. That keeps the client offline-capable and dependency-free,
and it is why this model validates ranges but knows nothing about places.

Shared by the CLI (`--geo lat,lng`) and the MCP proxy, where it is also the
tool schema the model fills in — so the field descriptions are prompt surface,
not just documentation.
"""

from __future__ import annotations

from pydantic import BaseModel, Field, field_validator


class GeoPoint(BaseModel):
    """A point in decimal degrees, latitude first."""

    lat: float = Field(description="WGS-84 latitude in decimal degrees, within [-90, 90].")
    lng: float = Field(description="WGS-84 longitude in decimal degrees, within [-180, 180].")

    @field_validator("lat")
    @classmethod
    def _lat_range(cls, v: float) -> float:
        if not -90.0 <= v <= 90.0:
            raise ValueError("lat must be within [-90, 90]")
        return v

    @field_validator("lng")
    @classmethod
    def _lng_range(cls, v: float) -> float:
        if not -180.0 <= v <= 180.0:
            raise ValueError("lng must be within [-180, 180]")
        return v
