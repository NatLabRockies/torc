#!/usr/bin/env bash
# Mock ReEDS step for the dynamic_orchestrator example. A real ReEDS run would
# read the previous PRAS output and write its own capacity-expansion result.
# Here we just record that the iteration ran.
set -euo pipefail

LINEAGE=${1:?lineage required}
GEN=${2:?generation required}

DEMO_DIR="${TORC_DEMO_DIR:-${TORC_OUTPUT_DIR:-$PWD/out}/dynamic_demo}"
WORK_DIR="$DEMO_DIR/$LINEAGE"
mkdir -p "$WORK_DIR"

echo "[reeds $LINEAGE gen=$GEN] running on $(hostname)"
# Pretend ReEDS does real work.
sleep "${TORC_DEMO_WORK_SECS:-1}"

OUT="$WORK_DIR/reeds_i$(printf '%02d' "$GEN").json"
jq -nc --arg lineage "$LINEAGE" --argjson gen "$GEN" \
    --arg host "$(hostname)" \
    '{lineage:$lineage, generation:$gen, host:$host, capacity:42}' > "$OUT"
echo "[reeds $LINEAGE gen=$GEN] wrote $OUT"
