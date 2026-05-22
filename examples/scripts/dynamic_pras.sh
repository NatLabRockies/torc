#!/usr/bin/env bash
# Mock PRAS step for the dynamic_orchestrator example. Real PRAS would consume
# the ReEDS capacity plan and write a reliability metric. Here we synthesize a
# metric that decays geometrically across iterations so the orchestrator's
# convergence test (metric < 0.01) trips after a few generations.
set -euo pipefail

LINEAGE=${1:?lineage required}
GEN=${2:?generation required}

DEMO_DIR="${TORC_DEMO_DIR:-${TORC_OUTPUT_DIR:-$PWD/out}/dynamic_demo}"
WORK_DIR="$DEMO_DIR/$LINEAGE"
mkdir -p "$WORK_DIR"

echo "[pras  $LINEAGE gen=$GEN] running on $(hostname)"
sleep "${TORC_DEMO_WORK_SECS:-1}"

# 0.5, 0.125, 0.03125, 0.0078125 -> converges by gen 4.
METRIC=$(awk -v g="$GEN" 'BEGIN{printf "%.6f", 0.5 * (0.25 ^ (g-1))}')
OUT="$WORK_DIR/pras_i$(printf '%02d' "$GEN").json"
jq -nc --arg lineage "$LINEAGE" --argjson gen "$GEN" \
    --argjson metric "$METRIC" \
    '{lineage:$lineage, generation:$gen, metric:$metric}' > "$OUT"
echo "[pras  $LINEAGE gen=$GEN] metric=$METRIC -> $OUT"
