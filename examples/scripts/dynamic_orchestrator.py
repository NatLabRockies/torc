#!/usr/bin/env python3
"""Dynamic-jobs orchestrator (ReEDS/PRAS feedback-loop example).

Each invocation of this script is one orchestrator generation in one lineage.
It inspects the previous iteration's PRAS output and either:

  * spawns the next reeds/pras pair plus a continuation of itself (all blocked
    on this orchestrator job), then exits 0 — the torc runner then completes
    this job and the normal unblock cascade promotes the spawned jobs; or
  * decides convergence: optionally writes a final state payload (no spawn)
    and exits 0; the workflow finishes naturally for this lineage.

Multiple seed orchestrator jobs (one per ReEDS case) yield independent
concurrent lineages — torc's resource packer interleaves all their reeds/pras
jobs across compute nodes.

Invocation:
    python3 dynamic_orchestrator.py <lineage>     # seed (first generation)

Spawned continuations inherit the lineage via `TORC_ORCHESTRATOR_LINEAGE_ID`,
which torc sets in the spawned job's environment, so argv is only used on the
seed.

Requires the torc Python client (`pip install torc-client`, published on PyPI).
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

from torc import Orchestrator, SpawnJobModel

CONVERGENCE_THRESHOLD = 0.01      # convergence rule — edit me


def main() -> None:
    # `Orchestrator.from_env()` reads TORC_API_URL / TORC_WORKFLOW_ID /
    # TORC_JOB_ID from the env, plus TORC_ORCHESTRATOR_LINEAGE_ID on
    # spawned continuations. On the seed invocation we supply the lineage
    # from argv as the fallback.
    seed_lineage = sys.argv[1] if len(sys.argv) > 1 else None
    orch = Orchestrator.from_env(lineage_fallback=seed_lineage)

    demo_root = Path(os.environ.get(
        "TORC_DEMO_DIR",
        os.environ.get("TORC_OUTPUT_DIR", str(Path.cwd() / "out")),
    ))
    work_dir = demo_root / "dynamic_demo" / orch.lineage
    work_dir.mkdir(parents=True, exist_ok=True)

    def log(msg: str) -> None:
        print(f"[orchestrator {orch.lineage}] {msg}", file=sys.stderr, flush=True)

    current_gen = orch.generation
    next_gen = current_gen + 1
    log(f"current_gen={current_gen} next_gen={next_gen}")

    # Read prior PRAS metric (convergence input).
    prior_metric: float | None = None
    if current_gen >= 1:
        prior_file = work_dir / f"pras_i{current_gen:02d}.json"
        if prior_file.exists():
            prior_metric = float(json.loads(prior_file.read_text())["metric"])
            log(f"prior pras metric: {prior_metric}")

    # =====================================================================
    # CONVERGENCE TEST — edit this block for your real criterion.
    # Real ReEDS would compare PRAS-derived reliability against a tolerance
    # (possibly per-region, with history). The demo's mock PRAS emits a
    # metric that decays geometrically, so a threshold suffices.
    # =====================================================================
    if prior_metric is not None and prior_metric < CONVERGENCE_THRESHOLD:
        log(f"converged at gen={current_gen} (metric={prior_metric}) -> no spawn")
        orch.converge(state={
            "converged": True,
            "final_metric": prior_metric,
            "generation": current_gen,
        })
        return

    # =====================================================================
    # SPAWN BLOCK — edit this to change the iteration's shape.
    # Pattern: next reeds (blocked on this orchestrator) -> next pras
    # (blocked on reeds) -> next orchestrator continuation (blocked on both).
    # The implicit "blocked on this orchestrator" edge is added by torc on
    # every spawned job — listing it in `depends_on` is not required.
    # =====================================================================
    script_dir = Path(__file__).resolve().parent
    me = script_dir / "dynamic_orchestrator.py"
    reeds = script_dir / "dynamic_reeds.sh"
    pras = script_dir / "dynamic_pras.sh"

    reeds_name = f"reeds_{orch.lineage}_i{next_gen:02d}"
    pras_name = f"pras_{orch.lineage}_i{next_gen:02d}"
    cont_name = f"orch_{orch.lineage}_g{next_gen:02d}"

    resp = orch.spawn(
        jobs=[
            # ReEDS: 8 CPU / 10 GB. Blocked only on this orchestrator
            # (auto-injected), so it starts as soon as we exit and the
            # runner completes us.
            SpawnJobModel(
                name=reeds_name,
                command=f"bash {reeds} {orch.lineage} {next_gen}",
                resource_requirements="reeds_rr",
                priority=1,
            ),
            # PRAS: 1 CPU / 120 GB. Higher priority so it unblocks the
            # next orchestrator generation sooner under contention.
            SpawnJobModel(
                name=pras_name,
                command=f"bash {pras} {orch.lineage} {next_gen}",
                resource_requirements="pras_rr",
                priority=10,
                depends_on=[reeds_name],
            ),
            # Continuation: fan-in on the iteration's outputs. Set
            # cancel_on_blocking_job_failure=False so a failed reeds/pras
            # still lets us run and decide what to do.
            SpawnJobModel(
                name=cont_name,
                command=f"python3 {me}",
                resource_requirements="orch_rr",
                priority=0,
                depends_on=[reeds_name, pras_name],
                cancel_on_blocking_job_failure=False,
            ),
        ],
        state={"generation": next_gen, "prior_metric": prior_metric},
    )
    log(f"spawned gen={next_gen}: {reeds_name} -> {pras_name} -> {cont_name} "
        f"(iteration={resp.iteration}) — exiting; runner will complete us")


if __name__ == "__main__":
    main()
