#!/bin/bash
# test_framework.sh — Assertion library and pass/fail tracking for Slurm integration tests.
#
# Source this file in each test_*.sh script. It provides:
#   - assert_eq, assert_ne, assert_contains, assert_gt, assert_ge
#   - Workflow-specific assertions (assert_workflow_complete, assert_all_jobs_completed, etc.)
#   - Pass/fail counters and a final summary printer

# ── Counters ──────────────────────────────────────────────────────────────────
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_SKIPPED=0
CURRENT_TEST=""
CURRENT_WF_ID=""

# Accumulated failures for the final report
declare -a FAILURE_MESSAGES=()

# ── Core helpers ──────────────────────────────────────────────────────────────

_pass() {
    TESTS_PASSED=$((TESTS_PASSED + 1))
    echo "  PASS: $1"
}

_fail() {
    TESTS_FAILED=$((TESTS_FAILED + 1))
    local msg="$1"
    echo "  FAIL: $msg"
    FAILURE_MESSAGES+=("[$CURRENT_TEST] $msg")
    _dump_debug_info
}

# _dump_debug_info
#   Dumps workflow debug info on assertion failure. Output goes to both console
#   (truncated) and a debug file in $RUN_DIR.
_dump_debug_info() {
    if [ -z "${CURRENT_WF_ID:-}" ] || [ -z "${RUN_DIR:-}" ]; then
        return 0
    fi
    local debug_file="$RUN_DIR/debug_${CURRENT_TEST}_wf${CURRENT_WF_ID}.txt"
    {
        echo "=== Debug info for $CURRENT_TEST (workflow $CURRENT_WF_ID) ==="
        echo ""
        echo "--- Workflow Summary ---"
        torc --url "$TORC_API_URL" reports summary "$CURRENT_WF_ID" 2>&1 || true
        echo ""
        echo "--- Jobs ---"
        torc --url "$TORC_API_URL" jobs list "$CURRENT_WF_ID" 2>&1 || true
        echo ""
        echo "--- Results ---"
        torc --url "$TORC_API_URL" -f json reports results "$CURRENT_WF_ID" 2>&1 || true
    } > "$debug_file" 2>&1
    echo "    (debug info saved to $debug_file)"
}

_skip() {
    TESTS_SKIPPED=$((TESTS_SKIPPED + 1))
    echo "  SKIP: $1"
}

# ── Generic assertions ────────────────────────────────────────────────────────

# assert_eq ACTUAL EXPECTED LABEL
assert_eq() {
    local actual="$1" expected="$2" label="$3"
    if [ "$actual" = "$expected" ]; then
        _pass "$label"
    else
        _fail "$label (expected '$expected', got '$actual')"
    fi
}

# assert_ne ACTUAL UNEXPECTED LABEL
assert_ne() {
    local actual="$1" unexpected="$2" label="$3"
    if [ "$actual" != "$unexpected" ]; then
        _pass "$label"
    else
        _fail "$label (expected not '$unexpected', got '$actual')"
    fi
}

# assert_contains HAYSTACK NEEDLE LABEL
assert_contains() {
    local haystack="$1" needle="$2" label="$3"
    if echo "$haystack" | grep -qF "$needle"; then
        _pass "$label"
    else
        _fail "$label (output does not contain '$needle')"
    fi
}

# assert_contains_regex HAYSTACK PATTERN LABEL
assert_contains_regex() {
    local haystack="$1" pattern="$2" label="$3"
    if echo "$haystack" | grep -qE "$pattern"; then
        _pass "$label"
    else
        _fail "$label (output does not match pattern '$pattern')"
    fi
}

# assert_not_contains HAYSTACK NEEDLE LABEL
assert_not_contains() {
    local haystack="$1" needle="$2" label="$3"
    if ! echo "$haystack" | grep -qF "$needle"; then
        _pass "$label"
    else
        _fail "$label (output unexpectedly contains '$needle')"
    fi
}

# assert_gt ACTUAL THRESHOLD LABEL  (actual > threshold, integer)
assert_gt() {
    local actual="$1" threshold="$2" label="$3"
    if [ "$actual" -gt "$threshold" ] 2>/dev/null; then
        _pass "$label"
    else
        _fail "$label (expected > $threshold, got '$actual')"
    fi
}

# assert_ge ACTUAL THRESHOLD LABEL  (actual >= threshold, integer)
assert_ge() {
    local actual="$1" threshold="$2" label="$3"
    if [ "$actual" -ge "$threshold" ] 2>/dev/null; then
        _pass "$label"
    else
        _fail "$label (expected >= $threshold, got '$actual')"
    fi
}

# assert_gt_float ACTUAL THRESHOLD LABEL  (actual > threshold, float via awk)
assert_gt_float() {
    local actual="$1" threshold="$2" label="$3"
    if awk "BEGIN { exit !($actual > $threshold) }"; then
        _pass "$label"
    else
        _fail "$label (expected > $threshold, got '$actual')"
    fi
}

# assert_true CONDITION_EXIT_CODE LABEL
assert_true() {
    local exit_code="$1" label="$2"
    if [ "$exit_code" -eq 0 ]; then
        _pass "$label"
    else
        _fail "$label (command returned exit code $exit_code)"
    fi
}

# ── Workflow assertions ───────────────────────────────────────────────────────

# assert_workflow_complete WF_ID
#   Checks that the workflow reached "complete" state.
assert_workflow_complete() {
    local wf_id="$1"
    local result
    result=$(torc --url "$TORC_API_URL" -f json workflows is-complete "$wf_id" 2>/dev/null)
    local is_complete
    is_complete=$(echo "$result" | jq -r '.is_complete // false')
    if [ "$is_complete" = "true" ]; then
        _pass "workflow $wf_id is complete"
    else
        _fail "workflow $wf_id is NOT complete"
    fi
}

# assert_workflow_canceled WF_ID
#   Checks that the workflow is marked as canceled.
assert_workflow_canceled() {
    local wf_id="$1"
    local result
    result=$(torc --url "$TORC_API_URL" -f json workflows is-complete "$wf_id" 2>/dev/null)
    local is_canceled
    is_canceled=$(echo "$result" | jq -r '.is_canceled // false')
    if [ "$is_canceled" = "true" ]; then
        _pass "workflow $wf_id is canceled"
    else
        _fail "workflow $wf_id is NOT canceled"
    fi
}

# assert_all_jobs_completed WF_ID EXPECTED_COUNT
#   Verifies every job has status "completed" and the count matches.
assert_all_jobs_completed() {
    local wf_id="$1" expected="$2"
    local jobs_json
    jobs_json=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null)
    local total completed
    total=$(echo "$jobs_json" | jq '.jobs | length')
    completed=$(echo "$jobs_json" | jq '[.jobs[] | select(.status == "completed")] | length')
    assert_eq "$completed" "$expected" "workflow $wf_id: $expected jobs completed"
    assert_eq "$total" "$expected" "workflow $wf_id: $expected total jobs"
}

# assert_job_status WF_ID JOB_NAME STATUS
assert_job_status() {
    local wf_id="$1" job_name="$2" expected_status="$3"
    local status
    status=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq -r ".jobs[] | select(.name == \"$job_name\") | .status")
    assert_eq "$status" "$expected_status" "job '$job_name' has status '$expected_status'"
}

# assert_job_failed WF_ID JOB_NAME
assert_job_failed() {
    local wf_id="$1" job_name="$2"
    assert_job_status "$wf_id" "$job_name" "failed"
}

# assert_return_code WF_ID JOB_NAME CODE
#   Checks that a specific job's most recent result has the given return code.
assert_return_code() {
    local wf_id="$1" job_name="$2" expected_code="$3"
    local job_id rc
    job_id=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq -r ".jobs[] | select(.name == \"$job_name\") | .id")
    rc=$(torc --url "$TORC_API_URL" -f json reports results "$wf_id" 2>/dev/null \
        | jq -r "[.results[] | select(.job_id == $job_id)] | sort_by(.attempt_id) | last | .return_code")
    assert_eq "$rc" "$expected_code" "job '$job_name' return code is $expected_code"
}

# assert_parse_logs_detect_oom WF_ID OUTPUT_DIR
#   Runs `torc slurm parse-logs` and checks for OOM-related output.
assert_parse_logs_detect_oom() {
    local wf_id="$1" output_dir="$2"
    local parse_output
    parse_output=$(torc --url "$TORC_API_URL" slurm parse-logs "$output_dir" --workflow-id "$wf_id" 2>&1) || true
    if echo "$parse_output" | grep -qiE "out.of.memory|oom-kill|oom_kill|killed process|exceeded memory|OUT_OF_MEMORY"; then
        _pass "parse-logs detected OOM for workflow $wf_id"
    else
        _fail "parse-logs did NOT detect OOM for workflow $wf_id"
    fi
}

# assert_logs_analyze_detect_oom WF_ID OUTPUT_DIR
#   Runs `torc logs analyze` and checks for OOM-related output.
assert_logs_analyze_detect_oom() {
    local wf_id="$1" output_dir="$2"
    local analyze_output
    analyze_output=$(torc --url "$TORC_API_URL" logs analyze "$output_dir" --workflow-id "$wf_id" 2>&1) || true
    if echo "$analyze_output" | grep -qiE "out.of.memory|oom-kill|oom_kill|killed process|exceeded memory|OUT_OF_MEMORY"; then
        _pass "logs analyze detected OOM for workflow $wf_id"
    else
        _fail "logs analyze did NOT detect OOM for workflow $wf_id"
    fi
}

# assert_parse_logs_detect_timeout WF_ID OUTPUT_DIR
#   Runs `torc slurm parse-logs` and checks for timeout-related output.
assert_parse_logs_detect_timeout() {
    local wf_id="$1" output_dir="$2"
    local parse_output
    parse_output=$(torc --url "$TORC_API_URL" slurm parse-logs "$output_dir" --workflow-id "$wf_id" 2>&1) || true
    if echo "$parse_output" | grep -qiE "timeout|time.limit|walltime|exceeded.*time|killed.*time"; then
        _pass "parse-logs detected timeout for workflow $wf_id"
    else
        _fail "parse-logs did NOT detect timeout for workflow $wf_id"
    fi
}

# assert_logs_analyze_detect_timeout WF_ID OUTPUT_DIR
#   Runs `torc logs analyze` and checks for timeout-related output.
assert_logs_analyze_detect_timeout() {
    local wf_id="$1" output_dir="$2"
    local analyze_output
    analyze_output=$(torc --url "$TORC_API_URL" logs analyze "$output_dir" --workflow-id "$wf_id" 2>&1) || true
    if echo "$analyze_output" | grep -qiE "timeout|time.limit|walltime|exceeded.*time|killed.*time"; then
        _pass "logs analyze detected timeout for workflow $wf_id"
    else
        _fail "logs analyze did NOT detect timeout for workflow $wf_id"
    fi
}

# assert_sacct_job_state WF_ID EXPECTED_STATE
#   Checks that `torc slurm sacct` output contains the expected state (e.g., OUT_OF_MEMORY, FAILED).
assert_sacct_job_state() {
    local wf_id="$1" expected_state="$2"
    local sacct_output
    sacct_output=$(torc --url "$TORC_API_URL" slurm sacct "$wf_id" 2>&1) || true
    if echo "$sacct_output" | grep -qiF "$expected_state"; then
        _pass "sacct shows $expected_state for workflow $wf_id"
    else
        _fail "sacct does NOT show $expected_state for workflow $wf_id (output: $(echo "$sacct_output" | head -5))"
    fi
}

# assert_multi_node_dispatch WF_ID EXPECTED_NODE_COUNT
#   Checks that job stdout logs mention at least N distinct hostnames.
assert_multi_node_dispatch() {
    local wf_id="$1" expected="$2"
    local jobs_json hostnames count
    jobs_json=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null)
    # Collect hostnames from job stdout
    hostnames=""
    while IFS= read -r job_id; do
        local stdout
        stdout=$(get_job_stdout "$wf_id" "$job_id")
        local host
        host=$(echo "$stdout" | grep -oP 'on \K\S+' | head -1 || true)
        if [ -n "$host" ]; then
            hostnames="$hostnames $host"
        fi
    done < <(echo "$jobs_json" | jq -r '.jobs[].id')
    count=$(echo "$hostnames" | tr ' ' '\n' | sort -u | grep -c . || echo 0)
    assert_ge "$count" "$expected" "workflow $wf_id dispatched to >= $expected distinct nodes (got $count)"
}

# assert_peak_cpu_nonzero WF_ID JOB_NAME
#   Checks that peak_cpu_percent > 0 in the results for this job.
assert_peak_cpu_nonzero() {
    local wf_id="$1" job_name="$2"
    local job_id peak_cpu
    job_id=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq -r ".jobs[] | select(.name == \"$job_name\") | .id")
    peak_cpu=$(torc --url "$TORC_API_URL" -f json results list "$wf_id" 2>/dev/null \
        | jq -r "[.results[] | select(.job_id == $job_id)] | sort_by(.attempt_id) | last | .peak_cpu_percent // 0")
    assert_gt_float "${peak_cpu:-0}" "0" "job '$job_name' peak_cpu_percent > 0 (got $peak_cpu)"
}

# assert_any_peak_cpu_nonzero WF_ID — at least one job in the workflow has peak_cpu > 0
assert_any_peak_cpu_nonzero() {
    local wf_id="$1"
    local max_peak_cpu
    max_peak_cpu=$(torc --url "$TORC_API_URL" -f json results list "$wf_id" 2>/dev/null \
        | jq -r '[.results[].peak_cpu_percent // 0] | max // 0')
    assert_gt_float "${max_peak_cpu:-0}" "0" "at least one job has peak_cpu_percent > 0 (max=$max_peak_cpu)"
}

# assert_peak_memory_nonzero WF_ID JOB_NAME
assert_peak_memory_nonzero() {
    local wf_id="$1" job_name="$2"
    local job_id peak_mem
    job_id=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq -r ".jobs[] | select(.name == \"$job_name\") | .id")
    peak_mem=$(torc --url "$TORC_API_URL" -f json results list "$wf_id" 2>/dev/null \
        | jq -r "[.results[] | select(.job_id == $job_id)] | sort_by(.attempt_id) | last | .peak_memory_bytes // 0")
    assert_gt "${peak_mem:-0}" "0" "job '$job_name' peak_memory_bytes > 0 (got $peak_mem)"
}

# assert_resource_utilization_flags_violation WF_ID
#   Checks that check-resource-utilization --include-failed reports violations.
assert_resource_utilization_flags_violation() {
    local wf_id="$1"
    local output
    output=$(torc --url "$TORC_API_URL" -f json reports check-resource-utilization "$wf_id" \
        --include-failed 2>/dev/null) || true
    local violation_count
    violation_count=$(echo "$output" | jq '.resource_violations_count // .failed_jobs_count // 0' 2>/dev/null || echo 0)
    assert_gt "$violation_count" "0" "check-resource-utilization flags violations for workflow $wf_id"
}

# assert_slurm_stats_available WF_ID
#   Checks that `torc slurm stats` returns non-empty data.
assert_slurm_stats_available() {
    local wf_id="$1"
    local stats_output
    stats_output=$(torc --url "$TORC_API_URL" -f json slurm stats "$wf_id" 2>&1) || true
    if [ -n "$stats_output" ] && [ "$stats_output" != "null" ] && [ "$stats_output" != "{}" ]; then
        _pass "slurm stats available for workflow $wf_id"
    else
        _fail "slurm stats not available for workflow $wf_id"
    fi
}

# assert_resource_metrics_db_has_data OUTPUT_DIR [WF_ID]
#   Checks that a resource_metrics_*.db file exists and has sample data.
#   If WF_ID is provided, only looks for DBs matching that workflow.
assert_resource_metrics_db_has_data() {
    local output_dir="$1"
    local wf_id="${2:-}"
    local db_file pattern
    if [ -n "$wf_id" ]; then
        pattern="resource_metrics_wf${wf_id}_*.db"
    else
        pattern="resource_metrics_*.db"
    fi
    db_file=$(find "$output_dir" -name "$pattern" -type f 2>/dev/null | head -1)
    if [ -z "$db_file" ]; then
        _fail "no $pattern file found in $output_dir"
        return
    fi
    _pass "resource_metrics DB exists: $db_file"
    if command -v sqlite3 &>/dev/null; then
        local count
        count=$(sqlite3 "$db_file" "SELECT COUNT(*) FROM job_resource_samples;" 2>/dev/null || echo 0)
        assert_gt "$count" "0" "resource_metrics DB has $count sample rows"
    else
        _skip "sqlite3 not available, cannot verify resource_metrics DB content"
    fi
}

# ── Summary ───────────────────────────────────────────────────────────────────

# print_test_summary
#   Prints the final pass/fail/skip counts and any failure messages.
#   Returns 0 if all passed, 1 otherwise.
print_test_summary() {
    local total=$((TESTS_PASSED + TESTS_FAILED + TESTS_SKIPPED))
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "  TEST SUMMARY"
    echo "═══════════════════════════════════════════════════════════════"
    echo "  Total:   $total"
    echo "  Passed:  $TESTS_PASSED"
    echo "  Failed:  $TESTS_FAILED"
    echo "  Skipped: $TESTS_SKIPPED"
    echo "═══════════════════════════════════════════════════════════════"

    if [ ${#FAILURE_MESSAGES[@]} -gt 0 ]; then
        echo ""
        echo "  FAILURES:"
        for msg in "${FAILURE_MESSAGES[@]}"; do
            echo "    - $msg"
        done
        echo ""
    fi

    if [ "$TESTS_FAILED" -gt 0 ]; then
        return 1
    fi
    return 0
}

# write_results_json FILE
#   Writes a machine-readable JSON summary.
write_results_json() {
    local file="$1"
    local failures_json="[]"
    if [ ${#FAILURE_MESSAGES[@]} -gt 0 ]; then
        failures_json=$(printf '%s\n' "${FAILURE_MESSAGES[@]}" | jq -R . | jq -s .)
    fi
    cat > "$file" <<EOJSON
{
  "total": $((TESTS_PASSED + TESTS_FAILED + TESTS_SKIPPED)),
  "passed": $TESTS_PASSED,
  "failed": $TESTS_FAILED,
  "skipped": $TESTS_SKIPPED,
  "failures": $failures_json
}
EOJSON
}
