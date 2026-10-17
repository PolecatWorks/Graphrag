*** Settings ***
Documentation     Verifies HaMS sidecar health/metrics probes and foundational Axum endpoints on a running Graphrag instance.
Library           ../lib/GraphragApiLibrary.py    be_base_url=${BE_BASE_URL}    hams_base_url=${HAMS_BE_URL}

*** Variables ***
${BE_BASE_URL}      http://localhost:8080
${HAMS_BE_URL}      http://localhost:8079

*** Test Cases ***
Verify HaMS Liveness Probe
    [Documentation]    Confirms the HaMS /health/live probe returns HTTP 200 OK.
    [Tags]    health    hams
    Verify Hams Liveness

Verify HaMS Readiness Probe
    [Documentation]    Confirms the HaMS /health/ready probe returns HTTP 200 OK after DB and LLM client initialization.
    [Tags]    health    hams
    Verify Hams Readiness

Verify HaMS Prometheus Metrics Exposure
    [Documentation]    Confirms the HaMS /metrics endpoint exposes Prometheus telemetry.
    [Tags]    metrics    hams
    ${metrics}=    Verify Hams Metrics
    Should Not Be Empty    ${metrics}

Verify Axum Hello Endpoint
    [Documentation]    Confirms the primary Axum HTTP router responds on GET /hello.
    [Tags]    api    foundation
    ${body}=    Get Hello Endpoint
    Should Be Equal As Strings    ${body}    Hello, World!
