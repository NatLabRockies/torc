"""Unit tests for `torc.orchestrator.Orchestrator`.

These tests stub the API client and the environment, so no torc server is
required. They focus on the behaviors the helper owns end-to-end:
env/argv resolution, generation discovery, and the spawn/converge request
shapes.
"""

from __future__ import annotations

from types import SimpleNamespace
from typing import Any
from unittest.mock import MagicMock

import pytest

from torc.openapi_client.models.spawn_job_model import SpawnJobModel
from torc.openapi_client.models.spawn_jobs_request import SpawnJobsRequest
from torc.openapi_client.models.spawn_jobs_response import SpawnJobsResponse
from torc.orchestrator import LINEAGE_RECORD_PREFIX, Orchestrator


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_api(user_data_items: list[Any] | None = None) -> MagicMock:
    """Build a stub api whose `list_user_data` returns the given names.

    `spawn_jobs` is left as a `MagicMock` so individual tests can assert on
    the request payload it received.
    """
    api = MagicMock()
    api.list_user_data.return_value = SimpleNamespace(items=user_data_items or [])
    api.spawn_jobs.return_value = SpawnJobsResponse(spawned_job_ids=[], iteration=0)
    return api


def _ud(name: str) -> SimpleNamespace:
    return SimpleNamespace(name=name)


# ---------------------------------------------------------------------------
# from_env
# ---------------------------------------------------------------------------


def test_from_env_reads_torc_coordinates(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("TORC_API_URL", "http://localhost:8080/torc-service/v1")
    monkeypatch.setenv("TORC_WORKFLOW_ID", "42")
    monkeypatch.setenv("TORC_JOB_ID", "7")
    monkeypatch.setenv("TORC_ORCHESTRATOR_LINEAGE_ID", "lineage-a")

    orch = Orchestrator.from_env()

    assert orch.workflow_id == 42
    assert orch.job_id == 7
    assert orch.lineage == "lineage-a"


def test_from_env_uses_lineage_fallback_when_env_var_unset(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("TORC_API_URL", "http://localhost:8080/torc-service/v1")
    monkeypatch.setenv("TORC_WORKFLOW_ID", "1")
    monkeypatch.setenv("TORC_JOB_ID", "2")
    monkeypatch.delenv("TORC_ORCHESTRATOR_LINEAGE_ID", raising=False)

    orch = Orchestrator.from_env(lineage_fallback="seed-lineage")

    assert orch.lineage == "seed-lineage"


def test_from_env_prefers_env_var_over_fallback(monkeypatch: pytest.MonkeyPatch) -> None:
    """A spawned continuation always has the env var set; the fallback
    must not silently shadow it (would mis-route lineage state)."""
    monkeypatch.setenv("TORC_API_URL", "http://localhost:8080/torc-service/v1")
    monkeypatch.setenv("TORC_WORKFLOW_ID", "1")
    monkeypatch.setenv("TORC_JOB_ID", "2")
    monkeypatch.setenv("TORC_ORCHESTRATOR_LINEAGE_ID", "real-lineage")

    orch = Orchestrator.from_env(lineage_fallback="ignored")

    assert orch.lineage == "real-lineage"


def test_from_env_raises_when_lineage_unresolvable(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("TORC_API_URL", "http://localhost:8080/torc-service/v1")
    monkeypatch.setenv("TORC_WORKFLOW_ID", "1")
    monkeypatch.setenv("TORC_JOB_ID", "2")
    monkeypatch.delenv("TORC_ORCHESTRATOR_LINEAGE_ID", raising=False)

    with pytest.raises(RuntimeError, match="lineage is unset"):
        Orchestrator.from_env()


@pytest.mark.parametrize("missing", ["TORC_API_URL", "TORC_WORKFLOW_ID", "TORC_JOB_ID"])
def test_from_env_raises_when_required_var_missing(
    monkeypatch: pytest.MonkeyPatch, missing: str
) -> None:
    monkeypatch.setenv("TORC_API_URL", "http://localhost:8080/torc-service/v1")
    monkeypatch.setenv("TORC_WORKFLOW_ID", "1")
    monkeypatch.setenv("TORC_JOB_ID", "2")
    monkeypatch.setenv("TORC_ORCHESTRATOR_LINEAGE_ID", "x")
    monkeypatch.delenv(missing, raising=False)

    with pytest.raises(RuntimeError, match=missing):
        Orchestrator.from_env()


# ---------------------------------------------------------------------------
# generation
# ---------------------------------------------------------------------------


def test_generation_is_zero_on_seed_invocation() -> None:
    api = _make_api(user_data_items=[])
    orch = Orchestrator(api=api, workflow_id=1, job_id=2, lineage="a")
    assert orch.generation == 0


def test_generation_returns_max_matching_generation() -> None:
    api = _make_api(
        user_data_items=[
            _ud(f"{LINEAGE_RECORD_PREFIX}a__g000001"),
            _ud(f"{LINEAGE_RECORD_PREFIX}a__g000003"),
            _ud(f"{LINEAGE_RECORD_PREFIX}a__g000002"),
        ]
    )
    orch = Orchestrator(api=api, workflow_id=1, job_id=2, lineage="a")
    assert orch.generation == 3


def test_generation_ignores_other_lineages_and_finals() -> None:
    api = _make_api(
        user_data_items=[
            _ud(f"{LINEAGE_RECORD_PREFIX}a__g000001"),
            _ud(f"{LINEAGE_RECORD_PREFIX}b__g000005"),  # other lineage
            _ud(f"{LINEAGE_RECORD_PREFIX}a__final"),  # convergence record
            _ud("some-user-data"),  # unrelated
        ]
    )
    orch = Orchestrator(api=api, workflow_id=1, job_id=2, lineage="a")
    assert orch.generation == 1


def test_generation_is_cached_until_spawn() -> None:
    api = _make_api(user_data_items=[_ud(f"{LINEAGE_RECORD_PREFIX}a__g000002")])
    orch = Orchestrator(api=api, workflow_id=1, job_id=2, lineage="a")
    assert orch.generation == 2
    assert orch.generation == 2  # cached, not re-fetched
    assert api.list_user_data.call_count == 1


def test_lineage_with_regex_special_chars_is_escaped() -> None:
    """A lineage containing `.`, `+`, etc. must not match other lineages
    via accidental regex semantics."""
    api = _make_api(
        user_data_items=[
            _ud(f"{LINEAGE_RECORD_PREFIX}a.b__g000007"),
            _ud(f"{LINEAGE_RECORD_PREFIX}axb__g000999"),  # would match `.` if not escaped
        ]
    )
    orch = Orchestrator(api=api, workflow_id=1, job_id=2, lineage="a.b")
    assert orch.generation == 7


# ---------------------------------------------------------------------------
# spawn
# ---------------------------------------------------------------------------


def test_spawn_forwards_jobs_and_state_to_api() -> None:
    api = _make_api()
    orch = Orchestrator(api=api, workflow_id=1, job_id=2, lineage="a")
    jobs = [
        SpawnJobModel(name="child1", command="echo 1"),
        SpawnJobModel(name="child2", command="echo 2", depends_on=["child1"]),
    ]

    orch.spawn(jobs=jobs, state={"k": "v"})

    api.spawn_jobs.assert_called_once()
    (called_job_id, called_request) = api.spawn_jobs.call_args.args
    assert called_job_id == 2
    assert isinstance(called_request, SpawnJobsRequest)
    assert called_request.lineage == "a"
    assert called_request.state == {"k": "v"}
    assert called_request.jobs == jobs


def test_spawn_invalidates_cached_generation() -> None:
    api = _make_api(user_data_items=[_ud(f"{LINEAGE_RECORD_PREFIX}a__g000001")])
    orch = Orchestrator(api=api, workflow_id=1, job_id=2, lineage="a")
    assert orch.generation == 1

    # After spawn, the next `generation` read should re-fetch (the server
    # advanced the counter).
    api.list_user_data.return_value = SimpleNamespace(
        items=[
            _ud(f"{LINEAGE_RECORD_PREFIX}a__g000001"),
            _ud(f"{LINEAGE_RECORD_PREFIX}a__g000002"),
        ]
    )
    orch.spawn(jobs=[SpawnJobModel(name="c", command="echo")])
    assert orch.generation == 2
    assert api.list_user_data.call_count == 2


# ---------------------------------------------------------------------------
# converge
# ---------------------------------------------------------------------------


def test_converge_sends_empty_jobs_with_state() -> None:
    api = _make_api()
    orch = Orchestrator(api=api, workflow_id=1, job_id=2, lineage="a")

    orch.converge(state={"final": True})

    api.spawn_jobs.assert_called_once()
    (_, req) = api.spawn_jobs.call_args.args
    assert req.jobs == []
    assert req.lineage == "a"
    assert req.state == {"final": True}


def test_converge_with_no_state() -> None:
    api = _make_api()
    orch = Orchestrator(api=api, workflow_id=1, job_id=2, lineage="a")
    orch.converge()

    (_, req) = api.spawn_jobs.call_args.args
    assert req.jobs == []
    assert req.state is None
