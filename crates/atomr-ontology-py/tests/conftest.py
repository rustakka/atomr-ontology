"""Shared pytest configuration: enable pytest-asyncio in auto mode."""

import pytest_asyncio  # noqa: F401

# Register the asyncio_mode setting so individual tests can use async def.
collect_ignore = []
