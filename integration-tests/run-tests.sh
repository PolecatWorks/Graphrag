#!/bin/bash
set -ex

# Ensure working directory is project root
if [ ! -d "integration-tests" ] && [ -d "../integration-tests" ]; then
    cd ..
fi

NS="${GARDEN_NAMESPACE:-default}"
echo "Using namespace: $NS"

POD_NAME="robot-test-runner"

# Ensure kubectl is in the path
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"

# Wait for the runner pod to be ready
kubectl wait --for=condition=Ready pod/$POD_NAME -n $NS --timeout=300s

# Install only pure-python dependencies into the runner pod
echo "Installing robotframework and requests into runner pod..."
kubectl exec $POD_NAME -n $NS -- /bin/bash -c "pip install robotframework robotframework-requests requests"
echo "pip install finished."

# Discover Graphrag Pod IP and Service
BE_POD_IP=$(kubectl get pods -l app=graphrag -n $NS -o jsonpath='{.items[0].status.podIP}' 2>/dev/null || echo "")
echo "Backend Pod IP: $BE_POD_IP"

# Prepare test directories inside the runner pod
kubectl exec $POD_NAME -n $NS -- rm -rf /tmp/robot-tests /tmp/reports /tmp/lib /tmp/tests /tmp/data
kubectl exec $POD_NAME -n $NS -- mkdir -p /tmp/robot-tests /tmp/reports /tmp/tests /tmp/tests/data/graph_rag_baseline /tmp/lib

# Copy test assets and baseline data to preserve relative path ../../tests/data/graph_rag_baseline
kubectl cp ./integration-tests/tests $POD_NAME:/tmp/robot-tests -n $NS
kubectl cp ./integration-tests/lib $POD_NAME:/tmp/robot-tests/lib -n $NS
kubectl cp ./integration-tests/lib $POD_NAME:/tmp/lib -n $NS
kubectl cp ./tests/data/graph_rag_baseline $POD_NAME:/tmp/tests/data/graph_rag_baseline -n $NS

# Execute Robot tests
TEST_EXIT_CODE=0
kubectl exec $POD_NAME -n $NS -- /bin/bash -c "
  export HOME=/home/pwuser
  cd /tmp
  export PATH=\$PATH:/home/pwuser/.local/bin:/home/pwuser/.venv/bin

  BE_BASE_URL=\"http://graphrag:80\"
  HAMS_BE_URL=\"http://${BE_POD_IP}:8079\"
  BASELINE_DATA_DIR=\"/tmp/tests/data/graph_rag_baseline\"

  robot --pythonpath /tmp/robot-tests/lib --pythonpath /tmp/lib \
    --variable BE_BASE_URL:\$BE_BASE_URL \
    --variable HAMS_BE_URL:\$HAMS_BE_URL \
    --variable BASELINE_DATA_DIR:\$BASELINE_DATA_DIR \
    --loglevel DEBUG \
    -d /tmp/reports /tmp/robot-tests
" || TEST_EXIT_CODE=$?

# Pull reports back
echo "Pulling reports back from the robot runner..."
rm -rf ./integration-tests/reports
mkdir -p ./integration-tests/reports
kubectl cp $NS/$POD_NAME:/tmp/reports ./integration-tests/reports || echo "Failed to copy reports"

exit $TEST_EXIT_CODE
