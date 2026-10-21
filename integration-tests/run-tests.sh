#!/bin/bash
set -e

NS=${GARDEN_NAMESPACE:-default}

echo "Waiting for robot-test-runner pod to be ready..."
kubectl wait --for=condition=Ready pod/robot-test-runner -n $NS --timeout=120s

echo "Discovering Graphrag Pod IP..."
BE_POD_IP=$(kubectl get pods -l app=graphrag -n $NS -o jsonpath='{.items[0].status.podIP}')
if [ -z "$BE_POD_IP" ]; then
  echo "Failed to discover Graphrag Pod IP"
  exit 1
fi

echo "Graphrag Pod IP is $BE_POD_IP"

echo "Copying test assets into runner..."
kubectl cp ./integration-tests/tests robot-test-runner:/tmp/tests -n $NS
kubectl cp ./integration-tests/lib robot-test-runner:/tmp/lib -n $NS
kubectl cp ./tests/data/graph_rag_baseline robot-test-runner:/tmp/data -n $NS

echo "Running tests in pod..."
kubectl exec -n $NS robot-test-runner -- sh -c "
  pip install robotframework robotframework-requests ragas langchain-openai datasets && \
  robot --outputdir /tmp/reports --variable GRAPHRAG_URL:http://graphrag:80 --variable HAMS_URL:http://$BE_POD_IP:8079 /tmp/tests
"

echo "Copying test reports back..."
mkdir -p ./integration-tests/reports
kubectl cp robot-test-runner:/tmp/reports ./integration-tests/reports -n $NS

echo "Tests completed successfully."