"""Helper for writing orchestrator scripts that use the `spawn_jobs` API.

An orchestrator script is a torc job whose body inspects prior state and
either (a) calls `spawn_jobs` to add the next iteration's jobs blocked on
itself, or (b) records a final state and exits. The `Orchestrator` class
absorbs the boilerplate every such script needs: discovering its torc
coordinates from the standard `TORC_*` environment variables, resolving
the lineage, deriving the current spawn-iteration counter, and wrapping
the typed API request models.

Typical usage::

    from torc import Orchestrator, SpawnJobModel

    orch = Orchestrator.from_env(lineage_fallback=sys.argv[1] if len(sys.argv) > 1 else None)
    if converged:
        orch.converge(state={"final_metric": metric})
    else:
        orch.spawn(
            jobs=[SpawnJobModel(name="child", command="...", resource_requirements="rr")],
            state={"generation": orch.generation + 1},
        )
"""

from __future__ import annotations

import os
import re
from typing import Any

from torc.api import DefaultApi, make_api
from torc.openapi_client.models.spawn_job_model import SpawnJobModel
from torc.openapi_client.models.spawn_jobs_request import SpawnJobsRequest
from torc.openapi_client.models.spawn_jobs_response import SpawnJobsResponse


# Prefix the server uses for the append-only per-generation `user_data`
# records that back `spawn_jobs`. Exposed so callers can reason about
# torc's lineage namespace if they need to, but typical orchestrators
# just use the `Orchestrator.generation` property.
LINEAGE_RECORD_PREFIX = "__torc_lineage__"

# Generous safety upper bound for paging `list_user_data`. The server's
# default `dynamic_jobs.max_iterations` is 1000; with one record per
# generation and a few non-lineage user_data rows, fetching 10k in one
# page covers any realistic lineage.
_USER_DATA_PAGE_LIMIT = 10_000


class Orchestrator:
    """Per-iteration context for a dynamic-jobs orchestrator script.

    Wraps the conventions torc establishes for orchestrator scripts:

    * Reads connection (``TORC_API_URL``), workflow (``TORC_WORKFLOW_ID``),
      and job (``TORC_JOB_ID``) coordinates from the environment variables
      torc sets on every job.
    * Resolves the lineage: from ``TORC_ORCHESTRATOR_LINEAGE_ID`` (set by
      torc on every spawned continuation) or, on the seed invocation,
      from a caller-supplied fallback (typically ``sys.argv[1]``).
    * Derives the current spawn-iteration counter for the lineage by
      consulting torc's append-only ``__torc_lineage__<lineage>__g######``
      ``user_data`` records, so callers never need to parse internal names.

    Use :meth:`spawn` to add the next generation of jobs and
    :meth:`converge` to record a final state without spawning. Both
    methods are thin wrappers over ``JobsApi.spawn_jobs``.
    """

    def __init__(
        self,
        api: DefaultApi,
        workflow_id: int,
        job_id: int,
        lineage: str,
    ) -> None:
        self.api = api
        self.workflow_id = workflow_id
        self.job_id = job_id
        self.lineage = lineage
        self._generation: int | None = None

    @classmethod
    def from_env(cls, *, lineage_fallback: str | None = None) -> Orchestrator:
        """Build an :class:`Orchestrator` from the torc-set environment variables.

        Parameters
        ----------
        lineage_fallback : str, optional
            Lineage to use on the seed invocation, when
            ``TORC_ORCHESTRATOR_LINEAGE_ID`` is not set. If omitted and
            the env var is also missing, raises :class:`RuntimeError`.
            Typical seed usage:
            ``Orchestrator.from_env(lineage_fallback=sys.argv[1])``.

        Returns
        -------
        Orchestrator
            Initialized orchestrator context. The generation counter is
            fetched lazily on first access to :attr:`generation`.
        """
        api_url = _require_env("TORC_API_URL")
        workflow_id = int(_require_env("TORC_WORKFLOW_ID"))
        job_id = int(_require_env("TORC_JOB_ID"))
        lineage = os.environ.get("TORC_ORCHESTRATOR_LINEAGE_ID") or lineage_fallback
        if not lineage:
            msg = (
                "Orchestrator lineage is unset: provide `lineage_fallback` "
                "(typically `sys.argv[1]`) on the seed invocation; spawned "
                "continuations inherit it via TORC_ORCHESTRATOR_LINEAGE_ID."
            )
            raise RuntimeError(msg)
        return cls(
            api=make_api(api_url),
            workflow_id=workflow_id,
            job_id=job_id,
            lineage=lineage,
        )

    @property
    def generation(self) -> int:
        """Current spawn-iteration counter for this lineage (0 on the seed).

        Derived from the ``__torc_lineage__<lineage>__g######`` user_data
        records the server appends on every successful
        ``spawn(jobs=...)`` call. Cached after the first read and
        invalidated by :meth:`spawn`.
        """
        if self._generation is None:
            self._generation = self._fetch_generation()
        return self._generation

    def _fetch_generation(self) -> int:
        prefix = f"{LINEAGE_RECORD_PREFIX}{self.lineage}__g"
        pattern = re.compile(rf"^{re.escape(prefix)}(\d+)$")
        records = self.api.list_user_data(self.workflow_id, limit=_USER_DATA_PAGE_LIMIT)
        generations = [
            int(m.group(1))
            for ud in (records.items or [])
            if (m := pattern.match(ud.name)) is not None
        ]
        return max(generations, default=0)

    def spawn(
        self,
        jobs: list[SpawnJobModel],
        *,
        state: Any = None,
    ) -> SpawnJobsResponse:
        """Add the next generation of jobs, blocked on this orchestrator.

        Parameters
        ----------
        jobs : list of SpawnJobModel
            Job names declared in this batch may reference each other via
            ``depends_on``; torc resolves them within the transaction.
        state : optional
            Opaque JSON-serializable state attached to this generation.
            Typically used as input to the next iteration's convergence
            test.

        Returns
        -------
        SpawnJobsResponse
            Carries the new job IDs and the post-call iteration counter.
            The cached :attr:`generation` is invalidated so the next read
            re-fetches.
        """
        resp = self.api.spawn_jobs(
            self.job_id,
            SpawnJobsRequest(lineage=self.lineage, jobs=jobs, state=state),
        )
        self._generation = None
        return resp

    def converge(self, *, state: Any = None) -> SpawnJobsResponse:
        """Record a final state for this lineage without spawning any jobs.

        Equivalent to ``spawn(jobs=[], state=state)``. The orchestrator
        job is completed by the runner when this script exits and the
        workflow finishes for this lineage.

        Parameters
        ----------
        state : optional
            Opaque JSON-serializable state to persist as the lineage's
            final record. ``None`` records the convergence with no
            payload.

        Returns
        -------
        SpawnJobsResponse
            ``spawned_job_ids`` is empty; ``iteration`` is the lineage's
            counter at convergence (unchanged by this call).
        """
        return self.api.spawn_jobs(
            self.job_id,
            SpawnJobsRequest(lineage=self.lineage, jobs=[], state=state),
        )


def _require_env(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        msg = f"Required environment variable {name!r} is unset"
        raise RuntimeError(msg)
    return value
