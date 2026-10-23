# GitHub CI, Multi-Arch Docker Publish & Ephemeral Integration Testing PRD

## Overview
This PRD defines the requirements and architecture for **Continuous Integration (CI)**, **Multi-Architecture Container Publishing (`docker-publish`)**, **Helm Chart OCI Packaging**, and **Ephemeral Kubernetes Integration Testing (`integration-test`)** in **GraphRAG-RS**, modeled directly after the production patterns in [`PolecatWorks/agent-as-data`](https://github.com/PolecatWorks/agent-as-data).

The goal is to guarantee that every code, chart, or test change is automatically validated via path-filtered PR checks, packaged into content-SHA-addressed multi-arch (`linux/amd64` + `linux/arm64`) OCI container images using `cargo-chef` caching, deployed into isolated per-PR Kubernetes namespaces via **Garden** and **Helm**, verified end-to-end by **Robot Framework + Ragas** suites with HTML reports published to GitHub Pages, and rolled out to the `graphrag-dev` cluster on merge to `main`.

---

## End-to-End CI/CD & Integration Pipeline Architecture

```mermaid
flowchart TD
    subgraph Triggers["GitHub Event Triggers"]
        PR["pull_request -> main"]
        MG["merge_group -> main"]
        PushMain["push -> refs/heads/main"]
        Cron["schedule (Daily 00:00 UTC)"]
    end

    subgraph PRValidation["Fast PR Validation (.github/workflows/)"]
        CI["ci.yml\n(dorny/paths-filter -> cargo check & test + helm lint)"]
        HelmPR["helm-ci-pr.yaml\n(helm lint charts/graphrag)"]
        SkipMQ["skip-merge-queue.yml\n(Fast status stubs on merge_group)"]
    end

    subgraph DockerPipeline["Multi-Arch Container Pipeline (docker-publish.yml)"]
        CheckSHA["check-image\nCompute Git Content SHA & Inspect GHCR"]
        BuildMatrix["build (Matrix: linux/amd64 + linux/arm64)\ncargo-chef + Buildx Registry Cache -> Push by Digest"]
        MergeManifest["merge\ndocker buildx imagetools create (sha-<CONTENT_SHA>)"]
        RetagMain["retag (on push to main)\nRetag existing sha-<CONTENT_SHA> -> :main & :latest"]
        RolloutDev["rollout-dev (polecatworks-runner)\nkubectl rollout restart deployment/graphrag -n graphrag-dev"]
    end

    subgraph K8sIntegration["Ephemeral K8s Integration Testing (integration-test.yaml)"]
        WaitImage["Wait for ghcr.io/polecatworks/graphrag:sha-<CONTENT_SHA>"]
        GardenDeploy["garden test --env pr (Namespace: graphrag-pr-<PR>)\nDeploy postgres-test (pgvector) + graphrag Helm + robot-test-runner"]
        RunRobot["integration-tests/run-tests.sh\nPod IP Discovery (:8079 HaMS) + Robot & Ragas Suites"]
        PublishPages["Publish HTML Reports to gh-pages (pr/<PR>/log.html)\n& Comment Report Link on PR"]
        TeardownNS["Teardown Ephemeral Namespace\ngarden cleanup namespace + kubectl delete ns"]
    end

    subgraph HelmAndRetention["Helm OCI & Package Retention"]
        HelmMain["helm-ci-main.yaml\nPush OCI Chart to ghcr.io/polecatworks/graphrag/helm"]
        Cleanup["cleanup-dev-packages.yml\nsnok/container-retention-policy (2-week cutoff)"]
    end

    PR --> CI
    PR --> HelmPR
    MG --> SkipMQ

    MG --> CheckSHA
    PushMain --> CheckSHA
    CheckSHA -->|"Image Missing"| BuildMatrix
    BuildMatrix --> MergeManifest
    CheckSHA -->|"Image Exists on push to main"| RetagMain
    MergeManifest -->|"on push to main"| RolloutDev
    RetagMain --> RolloutDev

    MG --> WaitImage
    WaitImage --> GardenDeploy
    GardenDeploy --> RunRobot
    RunRobot --> PublishPages
    PublishPages --> TeardownNS

    PushMain --> HelmMain
    Cron --> Cleanup
```

---

## Core Components & Requirements

### 1. Multi-Stage `cargo-chef` Container Build (`Dockerfile`)
**Objective:** Produce minimal, reproducible, multi-arch runtime images with deterministic dependency layer caching.
- **Builder Base (`lukemathwalker/cargo-chef:latest-rust-1-bookworm`):**
  - `chef` stage: Installs `pkg-config`, `cmake`, and `clang` with BuildKit apt cache mounts (`/var/cache/apt`, `/var/lib/apt`).
  - `planner` stage: Runs `cargo chef prepare --recipe-path recipe.json`.
  - `buildcache` / `dev` stage: Installs `cargo-watch` and pre-cooks dependencies (`cargo chef cook --recipe-path recipe.json`) for containerized local development.
  - `buildcacherelease` stage: Pre-compiles release dependencies (`RUSTFLAGS="--cfg tokio_unstable" cargo chef cook --release --recipe-path recipe.json`) with `/usr/local/cargo/registry` and `/usr/local/cargo/git` cache mounts.
  - `build` stage: Copies source tree and compiles the release binary (`cargo build --release`).
- **Runtime Image (`debian:bookworm-slim`):**
  - Installs only minimal runtime dependencies (`libssl3`, `ca-certificates`).
  - Copies the compiled binary (`/usr/local/bin/graphrag` / `/usr/local/bin/app`), `/app/migrations`, and default `/app/config.yaml`.
  - Verifies binary execution during build (`--help`).
  - Exposes ports `8080` (Axum REST & MCP HTTP JSON-RPC 2.0) and `8079` (HaMS health & Prometheus metrics sidecar).

---

### 2. Kubernetes Helm Chart (`charts/graphrag`) & Helm CI Workflows
**Objective:** Provide standardized Kubernetes deployment manifests for both ephemeral PR test environments and persistent dev/prod clusters.
- **Chart Structure (`charts/graphrag/`):**
  ```
  charts/graphrag/
  ├── Chart.yaml                  # Chart metadata (name: graphrag, version: 0.1.0)
  ├── values.yaml                 # Default image, service, resource, and config values
  ├── configs/
  │   └── config.yaml             # Mounted configuration template
  └── templates/
      ├── _helpers.tpl            # Labels, names, and config checksum helpers
      ├── configmap.yaml          # ConfigMap mounted into the container
      ├── deployment.yaml         # Deployment exposing http (:8080) and hams (:8079)
      └── service.yaml            # ClusterIP Service exposing port 80 (-> 8080) and hamsPort 8079
  ```
- **HaMS Health Probes in `deployment.yaml`:**
  - **Liveness Probe:** `HTTP GET /health/alive` on port `hams` (`8079`).
  - **Readiness Probe:** `HTTP GET /health/ready` on port `hams` (`8079`).
- **Helm CI Workflows:**
  1. **`.github/workflows/helm-ci-pr.yaml` (`on: pull_request`):**
     - Uses `dorny/paths-filter@v3` on `charts/**` and `.github/workflows/helm-ci-*.yaml`.
     - Runs `helm lint charts/graphrag` when chart files change, guarded by a `Helm CI Success` status job.
  2. **`.github/workflows/helm-ci-main.yaml` (`on: push` to `main`):**
     - Lints `charts/graphrag`, appends `-main-${TIMESTAMP}-${SHA_SHORT}` to `Chart.yaml` version, and pushes the packaged chart to `oci://ghcr.io/polecatworks/graphrag/helm` via `bsord/helm-push@v4`.

---

### 3. Pull Request CI Workflow (`ci.yml`) & Merge Queue Stubs (`skip-merge-queue.yml`)
**Objective:** Run fast, path-filtered compile, unit test, and lint checks on every Pull Request while avoiding redundant test execution in the GitHub Merge Queue.

- **`.github/workflows/ci.yml` (`on: pull_request`):**
  1. **`detect-changes` Job:** Uses `dorny/paths-filter@v3` with filters:
     - `backend`: `src/**`, `migrations/**`, `Cargo.toml`, `Cargo.lock`, `config.yaml`, `Makefile`, `.github/workflows/ci.yml`
     - `helm`: `charts/**`, `.github/workflows/ci.yml`
  2. **`backend-test` (`Build & Test Backend`) Job:**
     - Runs conditionally when `needs.detect-changes.outputs.backend == 'true'`.
     - Starts a `pgvector/pgvector:pg16` service container on port `5432` (`POSTGRES_USER=postgres`, `POSTGRES_PASSWORD=postgres`, `POSTGRES_DB=graphrag_db`) with `pg_isready` health checks.
     - Configures `dtolnay/rust-toolchain@stable` and `Swatinem/rust-cache@v2`.
     - Runs `cargo check` and `cargo test -- --test-threads=1` with `DATABASE_URL=postgres://postgres:postgres@localhost:5432/graphrag_db`.
  3. **`helm-lint` (`Helm Chart Lint`) Job:**
     - Runs conditionally when `needs.detect-changes.outputs.helm == 'true'`.
     - Uses `azure/setup-helm@v4` and runs `helm lint charts/graphrag`.
- **`.github/workflows/skip-merge-queue.yml` (`on: merge_group`):**
  - Emits immediate passing status checks for `Build & Test Backend`, `Helm Chart Lint`, and `Helm CI Success` during `merge_group` evaluation because unit tests and chart linting already passed on the PR, leaving the merge queue to focus on container image verification and Kubernetes integration testing.

---

### 4. Multi-Arch Docker Build, Retag & Dev Rollout (`docker-publish.yml` & `cleanup-dev-packages.yml`)
**Objective:** Build multi-architecture (`linux/amd64` and `linux/arm64`) container images indexed by Git content SHA, avoid rebuilding unchanged source trees on merge to `main`, and automatically roll out updates to the `graphrag-dev` Kubernetes namespace.

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

- **Key Jobs in `.github/workflows/docker-publish.yml`:**
  1. **`check-paths`**: Filters on `src/**`, `migrations/**`, `Cargo.toml`, `Cargo.lock`, `config.yaml`, `Dockerfile`, and `.github/workflows/docker-publish.yml`.
  2. **`check-image`**: Computes `CONTENT_SHA` via `git log -1 --format=%H` across backend paths and checks whether `ghcr.io/polecatworks/graphrag:sha-${CONTENT_SHA}` already exists in GHCR.
  3. **`build`**: Matrix build over `linux/amd64` (`ubuntu-24.04`) and `linux/arm64` (`ubuntu-24.04-arm`) when `exists != 'true'`, pushing image layers by digest with registry caching (`:buildcache-linux-amd64`, `:buildcache-linux-arm64`) and GitHub Actions cache (`type=gha`), then uploading digest files via `actions/upload-artifact@v6`.
  4. **`merge`**: Downloads platform digests and runs `docker buildx imagetools create` with OCI annotations to publish the combined multi-arch manifest list tagged with `sha-${CONTENT_SHA}` (and `main`, `latest` when on `main`).
  5. **`retag`**: When `github.event_name == 'push'` on `refs/heads/main` and `exists == 'true'`, retags the pre-built `sha-${CONTENT_SHA}` manifest as `:main` and `:latest` in seconds without recompiling Rust code.
  6. **`rollout-dev`**: Runs on `polecatworks-runner` after `merge` or `retag` on `main` and executes `kubectl rollout restart deployment/graphrag -n graphrag-dev`.
  7. **`ci-success`**: Final required status check (`Graphrag Docker Success`) gating merge queue and branch protection.
- **Package Retention (`.github/workflows/cleanup-dev-packages.yml`):**
  - Runs daily (`0 0 * * *`) and via `workflow_dispatch` using `snok/container-retention-policy@v2.2.1` to prune `graphrag` and `graphrag/helm/graphrag` packages older than `2 weeks ago UTC`, skipping `main`, `latest`, `*.*.*`, and `v*.*.*`.

---

### 5. Ephemeral Kubernetes Integration Testing (`integration-test.yaml` & Garden)
**Objective:** Execute the full **Robot Framework + Ragas** integration test suite inside an isolated, ephemeral Kubernetes namespace (`graphrag-pr-<PR_NUMBER>`) against the exact container image built for the PR/merge group, publish rich HTML test logs to GitHub Pages, and post a direct report link on the PR.

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

- **Garden Project & Test Configuration (`garden.yml` & `integration-tests/garden.yml`):**
  - Defines `local` (`graphrag-${USER}`) and `pr` (`graphrag-pr-${PR_NUMBER}`) environments.
  - **`ghcr-external-secret` (`kind: Deploy`, `type: kubernetes`):** Provisions `ghcr-secret` for pulling images from `ghcr.io`.
  - **`postgres-test` (`kind: Deploy`, `type: kubernetes`):** Deploys `pgvector/pgvector:pg16` with `init.sql` (`CREATE EXTENSION IF NOT EXISTS vector;`).
  - **`graphrag` (`kind: Deploy`, `type: helm`):** Deploys `charts/graphrag` wired to `postgres-test:5432` and the resolved `GRAPHRAG_IMAGE:GRAPHRAG_TAG` (falling back to a `get-be-sha` `Build` exec action for local Garden runs).
  - **`robot-test-runner` (`kind: Deploy`, `type: kubernetes`):** Deploys the test runner pod (`marketsquare/robotframework-browser:latest` or Python Robot + Ragas runner) inside the namespace.
  - **`integration-all` (`kind: Test`, `type: exec`):** Invokes `./integration-tests/run-tests.sh` with `GARDEN_NAMESPACE=${environment.namespace}`.
- **Dual-Mode Test Execution & Pod-Direct Telemetry Resolution:**
  - **Local Mode (`./integration-tests/run-tests-local.sh` / `make test-robot`):** Targets `http://localhost:8080` (Graphrag API & MCP) and `http://localhost:8079` (HaMS `/hams/alive`, `/hams/ready`, `/hams/metrics`).
  - **In-Cluster Kubernetes Mode (`./integration-tests/run-tests.sh`):**
    1. Waits for `pod/robot-test-runner` to reach `Ready`.
    2. Installs required Python libraries (`robotframework`, `robotframework-requests`, `requests`) inside the runner pod. Dynamic dependency installation must avoid packages with C-extensions (such as full scientific `ragas` or `scikit-network` requiring C++ compilers like `g++`). Heavy ML evaluation libraries must be either baked into custom runner images or replaced by deterministic lightweight fallback evaluation routines in `RagasEvalLibrary.py`.
    3. Queries Kubernetes for the `graphrag` Pod IP (`kubectl get pods -l app=graphrag -n $NS -o jsonpath='{.items[0].status.podIP}'`) and passes `--variable BE_BASE_URL:http://graphrag:80 --variable HAMS_BE_URL:http://$BE_POD_IP:8079` to `robot` so health and Prometheus metrics assertions target the pod instance directly rather than load-balancing across a Service VIP.
    4. Copies `./integration-tests/tests`, `./integration-tests/lib`, and `./integration-tests/data/graph_rag_baseline` into `/tmp` on `robot-test-runner`. Test baseline data must be colocated under `integration-tests/data/` to ensure path filter detection triggers integration test runs and eliminates runner path lookup failures. Executes all `.robot` suites, and copies `/tmp/reports` back to `./integration-tests/reports`.
- **Report Artifacts, GitHub Pages Deployment & PR Commenting (`integration-test.yaml`):**
  - **Action Version Standard**: Standardizes on `actions/upload-artifact@v6` and `actions/download-artifact@v6`.
  - **Self-Hosted Runner Network Resilience**: The `Upload Robot Test Results Artifact` step specifies `continue-on-error: true`. When executed on self-hosted Kubernetes runners (`polecatworks-runner`), egress or DNS resolution to GitHub's Azure blob storage backend (`ENOTFOUND`) is treated as non-fatal.
  - **Authoritative Report Channel**: The primary and authoritative delivery channel for integration test results is Git-backed publication to GitHub Pages (`gh-pages` branch at `pr/${PR_NUMBER}/log.html`), supplemented with an idempotent PR markdown comment.
  - Performs a sparse checkout of the `gh-pages` branch (creating an orphan `gh-pages` branch if none exists yet), publishes the HTML report bundle to `pr/${PR_NUMBER}/` with retry-on-push rebase resilience, and posts an idempotent comment on the PR linking to `https://polecatworks.github.io/graphrag/pr/${PR_NUMBER}/log.html`.
  - Always cleans up the ephemeral namespace in a final `if: always()` step (`garden cleanup namespace --env pr` and `kubectl delete namespace graphrag-pr-${PR_NUMBER} --ignore-not-found`).

---

## Developer Makefile Additions
To maintain parity with `agent-as-data`, the root `Makefile` should also include container and Garden convenience targets alongside existing local targets:
- `make docker` / `make build-docker`: Build `graphrag:latest` locally from `Dockerfile`.
- `make docker-run`: Run `graphrag:latest` locally mapping `8080` and `8079`.
- `make helm-lint`: Run `helm lint charts/graphrag`.
- `make garden-up`: Deploy the stack to the local Kubernetes context via `garden deploy`.
- `make garden-test`: Run the full in-cluster integration test suite via `garden test`.

---

## Acceptance Criteria
1. **Multi-Stage `Dockerfile`**: Builds cleanly with `cargo-chef` caching, runs unit tests during build, and produces a slim Debian runtime image exposing ports `8080` and `8079`.
2. **Helm Chart (`charts/graphrag`)**: Passes `helm lint charts/graphrag` and configures HaMS liveness/readiness probes on port `8079`.
3. **GitHub Actions Workflows (`.github/workflows/`)**:
   - `ci.yml` runs path-filtered `cargo check`, `cargo test -- --test-threads=1` (with `pgvector/pgvector:pg16` service container), and `helm lint`.
   - `skip-merge-queue.yml` satisfies PR status checks on `merge_group`.
   - `docker-publish.yml` builds multi-arch (`linux/amd64`, `linux/arm64`) images tagged with `sha-<CONTENT_SHA>`, retags `:main`/`:latest` on push to `main`, and triggers `kubectl rollout restart` in `graphrag-dev`.
   - `helm-ci-pr.yaml`, `helm-ci-main.yaml`, and `cleanup-dev-packages.yml` automate chart linting, OCI chart publishing, and 2-week package retention.
   - `integration-test.yaml`, `garden.yml`, `integration-tests/garden.yml`, and `integration-tests/run-tests.sh` deploy ephemeral PR namespaces, execute Robot Framework + Ragas tests with pod-direct HaMS resolution, publish HTML reports to `gh-pages`, comment the report URL on the PR, and tear down the namespace.
