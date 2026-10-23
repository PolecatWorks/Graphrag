#!/bin/bash
set -e

# Local test runner for Graphrag Robot Framework + Ragas integration tests.
# Expects:
#   - Graphrag backend already running on localhost:8080
#   - HaMS sidecar running on localhost:8079

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

LOCAL_BE_URL="${LOCAL_BE_URL:-http://localhost:8080}"
LOCAL_HAMS_BE_URL="${LOCAL_HAMS_BE_URL:-http://localhost:8079}"
LLM_BASE_URL="${LLM_BASE_URL:-http://localhost:11434/v1}"
LLM_MODEL="${LLM_MODEL:-llama3.1:8b}"
BASELINE_DATA_DIR="${BASELINE_DATA_DIR:-${SCRIPT_DIR}/data/graph_rag_baseline}"

REPORT_DIR="${SCRIPT_DIR}/reports"
rm -rf "${REPORT_DIR}"
mkdir -p "${REPORT_DIR}"

RAW_TEST_PATH="${1:-${SCRIPT_DIR}/tests}"
if [[ "${RAW_TEST_PATH}" = /* ]]; then
    TEST_PATH="${RAW_TEST_PATH}"
else
    TEST_PATH="$(pwd)/${RAW_TEST_PATH}"
fi

# Detect robot executable (local venv, system, or sibling venv)
if [ -f "${SCRIPT_DIR}/.venv/bin/robot" ]; then
    ROBOT_CMD="${SCRIPT_DIR}/.venv/bin/robot"
elif command -v robot > /dev/null 2>&1; then
    ROBOT_CMD="robot"
elif [ -f "/Users/bengreene/Development/polecatworks/sward-warden/.venv/bin/robot" ]; then
    ROBOT_CMD="/Users/bengreene/Development/polecatworks/sward-warden/.venv/bin/robot"
else
    echo "Error: robot command not found. Install integration-tests/requirements.txt first."
    exit 1
fi

echo "=============================================="
echo " Graphrag Robot + Ragas Tests - Local Runner"
echo "=============================================="
echo "Backend URL:   ${LOCAL_BE_URL}"
echo "HaMS BE URL:   ${LOCAL_HAMS_BE_URL}"
echo "LLM Base URL:  ${LLM_BASE_URL}"
echo "LLM Model:     ${LLM_MODEL}"
echo "Baseline Dir:  ${BASELINE_DATA_DIR}"
echo "Test Path:     ${TEST_PATH}"
echo "Report Dir:    ${REPORT_DIR}"
echo "=============================================="

echo ""
echo "Running pre-flight checks against running Graphrag instance..."

if curl -sf "${LOCAL_HAMS_BE_URL}/hams/alive" > /dev/null 2>&1; then
    echo "  ✓ HaMS liveness is responding at ${LOCAL_HAMS_BE_URL}/hams/alive"
else
    echo "  ✗ Error: Graphrag HaMS sidecar is NOT responding at ${LOCAL_HAMS_BE_URL}/hams/alive."
    echo "    Please ensure Graphrag is running before invoking 'make test-robot'."
    exit 1
fi

if curl -sf "${LOCAL_HAMS_BE_URL}/hams/ready" > /dev/null 2>&1; then
    echo "  ✓ HaMS readiness is responding at ${LOCAL_HAMS_BE_URL}/hams/ready"
else
    echo "  ✗ Error: Graphrag HaMS readiness check failed at ${LOCAL_HAMS_BE_URL}/hams/ready."
    exit 1
fi

echo ""
echo "Starting Robot Framework + Ragas tests..."
echo ""

export PYTHONPATH="${SCRIPT_DIR}/lib:${PYTHONPATH}"
export LLM_BASE_URL="${LLM_BASE_URL}"
export LLM_MODEL="${LLM_MODEL}"

"${ROBOT_CMD}" \
    --variable BE_BASE_URL:${LOCAL_BE_URL} \
    --variable HAMS_BE_URL:${LOCAL_HAMS_BE_URL} \
    --variable LLM_BASE_URL:${LLM_BASE_URL} \
    --variable LLM_MODEL:${LLM_MODEL} \
    --variable BASELINE_DATA_DIR:${BASELINE_DATA_DIR} \
    --loglevel DEBUG \
    -d "${REPORT_DIR}" \
    "${TEST_PATH}"
