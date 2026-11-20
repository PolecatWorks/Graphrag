# GraphRAG Keycloak Client Claims & MCP Service Account Authentication PRD

## Overview
This PRD defines the authentication and claims contract for **GraphRAG (`graphrag-rs`)**, specifying how GraphRAG:
1. **Acts as a Resource Server (`graphrag-api`)**: Validating incoming user and machine tokens via cluster Keycloak JWKS.
2. **Defines Domain-Specific Claims & Roles**: Isolated under `resource_access['graphrag-api']` and custom `graphrag` claims.
3. **Authenticates Upstream & Downstream MCP Clients**: Accepting authorized service accounts (such as `angry-agent-sa`) and invoking downstream data services with its own service account (`graphrag-sa`).

---

## Architecture Topology

```mermaid
flowchart LR
    subgraph Clients ["Callers"]
        UserBrowser["MFE Shell / UI (End-User Token)"]
        AngryAgent["Angry-Agent Backend (angry-agent-sa Token)"]
    end

    subgraph Identity ["Keycloak (auth.svc)"]
        JWKS["JWKS Certs Endpoint"]
        Client_API["KeycloakClient: graphrag-api\n(Bearer-Only)"]
        Client_SA["KeycloakClient: graphrag-sa\n(Confidential / Service Account)"]
    end

    subgraph Service ["GraphRAG Backend (graphrag-rs)"]
        AuthLayer["Axum / Tower Auth Middleware"]
        GraphEngine["Ontology, Knowledge Graph & Hybrid Search"]
    end

    Clients -->|Bearer JWT (aud: graphrag-api)| AuthLayer
    AuthLayer -->|Verify Signature & Claims| JWKS
    AuthLayer -->|Authorized Context| GraphEngine
    GraphEngine -->|M2M calls with graphrag-sa token| Keycloak
```

---

## Specification Details

### 1. Keycloak Client Definition (`KeycloakClient: graphrag-api`)
- **Client ID**: `graphrag-api`
- **Type**: Bearer-only / Resource Server
- **Audience**: `aud: ["graphrag-api"]`
- **Client Roles**:
  - `graph:read`: Query knowledge graphs, execute vector searches, retrieve ontology definitions.
  - `graph:write`: Ingest entities, create relations, trigger graph construction pipelines.
  - `graph:admin`: Manage schema taxonomies, wipe and re-index graph partitions.

### 2. Unique Token Claims for GraphRAG
```json
{
  "iss": "http://keycloak-keycloakx-http.auth.svc.cluster.local/auth/realms/sw-dev",
  "sub": "4a761e82-2d17-4861-8290-79883bfad211",
  "aud": ["graphrag-api"],
  "resource_access": {
    "graphrag-api": {
      "roles": ["graph:read", "graph:write"]
    }
  },
  "graphrag": {
    "allowed_domains": ["finance", "telemetry", "provenance"],
    "max_traversal_depth": 3,
    "vector_search_limit": 50
  }
}
```

### 3. Service Account Integration (`graphrag-sa`)
- Used for machine-to-machine interactions when GraphRAG queries database engines or Kafka topics.
- Confidential client with credentials managed via ExternalSecrets.
