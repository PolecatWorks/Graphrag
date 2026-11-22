# GraphRAG Keycloak Service Claims & Auth Addendum PRD

> [!IMPORTANT]
> **Canonical Auth Pattern**: Global Keycloak OIDC (`shell-ui` + `ecosystem-audiences`), `@polecatworks/mfe-shared` token distribution, zero-fake-auth rules, dynamic JWKS verification, and M2M `client_credentials` mechanics are defined canonically in [**Mfe-shell: Ecosystem Keycloak OIDC, Claims Distribution & Service Auth Pattern PRD**](https://github.com/PolecatWorks/Mfe-shell/blob/main/spec/prds/keycloak-oidc-and-claims-distribution-prd.md).
>
> This document specifies only the **GraphRAG (`graphrag-container`)** service-local contract.

---

## 1. Keycloak Client & Roles (`graphrag-api`)

- **Client ID (`aud`)**: `graphrag-api`
- **Type**: Bearer-only / Resource Server (declared in `fluxcd-dev/keycloak-crs.yaml`)
- **Client Roles** (`resource_access["graphrag-api"].roles`):
  - `graph:read`: Query knowledge graphs, execute hybrid/vector searches, retrieve ontology definitions and subgraphs.
  - `graph:write`: Ingest documents/entities, steward relations/fields, and trigger graph/community construction pipelines.
  - `graph:admin`: Manage schema taxonomies, wipe and re-index graph partitions.

---

## 2. Service-Specific Token Claims (`graphrag`)

```json
{
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

---

## 3. Service-Local Integration Notes

- **Backend (`graphrag-container`, Rust/Axum)**: Protects all BREAD REST (`/api/graphrag/v1/*`) and MCP (`/mcp`, `/api/graphrag/v1/mcp`) endpoints via the canonical JWKS Tower middleware (`aud` contains `graphrag-api`), accepting end-user tokens from `Mfe-shell` and M2M tokens from `angry-agent-sa` / `agent-as-data-sa`.
- **Outbound M2M (`graphrag-sa`)**: Confidential client (credentials via ExternalSecrets) for outbound service-to-service interactions.
