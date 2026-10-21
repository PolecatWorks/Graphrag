# Specification 18: CI/CD Container Integration

**Status:** complete

## Overview
This specification details the CI/CD pipeline, multi-architecture Docker build, Helm chart configuration, and ephemeral Kubernetes integration testing environment for the Graph RAG project, as defined in `spec/prds/ci-cd-container-integration-prd.md`.

## Diagrams

### Docker Build & Publish Workflow
```mermaid
sequenceDiagram
    autonumber
    participant GH as GitHub Actions (merge_group / push main)
    participant Filter as check-paths & check-image
    participant Builders as Buildx Matrix (ubuntu-24.04 & ubuntu-24.04-arm)
    participant GHCR as GitHub Container Registry (ghcr.io)
    participant K8s as polecatworks-runner (graphrag-dev)

    GH->>Filter: Compute CONTENT_SHA = git log -1 --format=%H -- src/ migrations/ Cargo.* Dockerfile
    Filter->>GHCR: docker manifest inspect ghcr.io/polecatworks/graphrag:sha-<CONTENT_SHA>
    alt Image Does Not Exist in GHCR
        Filter->>Builders: Launch parallel linux/amd64 & linux/arm64 builds
        Builders->>GHCR: Push per-arch image digests + update :buildcache-<arch>
        Builders->>GH: Upload digest artifacts
        GH->>GHCR: merge job: docker buildx imagetools create -t :sha-<CONTENT_SHA> (and :main, :latest if main)
    else Image Already Exists (e.g. Built in Merge Queue, Now Pushed to main)
        Filter->>GHCR: retag job: docker buildx imagetools create -t :main -t :latest :sha-<CONTENT_SHA>
    end
    opt Event is push to refs/heads/main
        GH->>K8s: rollout-dev job: kubectl rollout restart deployment/graphrag -n graphrag-dev
    end
```

### Ephemeral Kubernetes Integration Testing
```mermaid
sequenceDiagram
    autonumber
    participant GHA as integration-test.yaml (polecatworks-runner)
    participant GHCR as GHCR Registry
    participant Garden as Garden CLI (garden.yml)
    participant K8s as K8s Namespace (graphrag-pr-<PR>)
    participant RunnerPod as robot-test-runner Pod
    participant Pages as GitHub Pages (gh-pages) & PR Comment

    GHA->>GHA: Resolve GRAPHRAG_IMAGE & GRAPHRAG_TAG (sha-<CONTENT_SHA> or main)
    GHA->>GHCR: Poll docker manifest inspect until image is available (up to 30m)
    GHA->>K8s: Create namespace graphrag-pr-<PR> & ghcr-secret
    GHA->>Garden: garden test --env pr --var graphrag-image=... --var graphrag-tag=...
    Garden->>K8s: Deploy postgres-test (pgvector:pg16) + graphrag Helm chart + robot-test-runner Pod
    Garden->>RunnerPod: Execute ./integration-tests/run-tests.sh
    RunnerPod->>K8s: Discover Graphrag Pod IP (app=graphrag) for direct HaMS :8079 telemetry
    RunnerPod->>K8s: Run Robot Framework + Ragas suites against http://graphrag:80 & http://<pod_ip>:8079
    RunnerPod-->>GHA: kubectl cp /tmp/reports -> ./robot-results (output.xml, log.html, report.html)
    GHA->>Pages: Sparse-checkout gh-pages & push ./robot-results to pr/<PR>/
    GHA->>Pages: Post PR comment with link to https://polecatworks.github.io/graphrag/pr/<PR>/log.html
    GHA->>K8s: Always teardown: garden cleanup namespace --env pr & kubectl delete ns
```

## Requirements

### 1. Multi-Stage `Dockerfile`
- Base build image: `lukemathwalker/cargo-chef:latest-rust-1-bookworm`
- Stages: `chef`, `planner`, `buildcache`, `dev`, `buildcacherelease`, `build`
- Runtime image: `debian:bookworm-slim`
- Installed runtime dependencies: `libssl3`, `ca-certificates`
- Copies `/app/target/release/app` to `/usr/local/bin/app`
- Exposes ports `8080` and `8079`
- `ENTRYPOINT ["/usr/local/bin/app"]` and `CMD ["-c", "/app/config.yaml"]`

### 2. Helm Chart (`charts/graphrag`)
- `Chart.yaml`: `name: graphrag`, version `0.1.0`
- `values.yaml`: Image `ghcr.io/polecatworks/graphrag:latest`, ports `8080`/`8079`
- `deployment.yaml`: Defines container deployment, configuring HaMS liveness (`/health/alive`) and readiness (`/health/ready`) probes on port `8079`.

### 3. GitHub Actions Workflows
- `.github/workflows/ci.yml`: Runs on PR. Triggers backend and Helm jobs based on paths.
- `.github/workflows/skip-merge-queue.yml`: Emits fast-passing status checks on `merge_group`.
- `.github/workflows/docker-publish.yml`: Multi-arch matrix build using buildx caching, merges and pushes digest tagged by content SHA, retags on push to main, restarts K8s rollout.
- `.github/workflows/helm-ci-pr.yaml` and `helm-ci-main.yaml`: Helm chart linting and publishing to OCI registry.
- `.github/workflows/cleanup-dev-packages.yml`: Cleans up ghcr.io images.
- `.github/workflows/integration-test.yaml`: Sets up ephemeral test namespace, resolves image, runs tests with garden, publishes reports to `gh-pages` and comments on PR.

### 4. Garden Configuration
- `garden.yml`: Defines environments `local` and `pr`, deploys `postgres-test` and `graphrag` helm chart.
- `integration-tests/garden.yml`: Defines `robot-test-runner` pod and `integration-all` test action.
- `integration-tests/run-tests.sh`: Helper script to discover pod IPs and execute test runner inside the runner pod.
