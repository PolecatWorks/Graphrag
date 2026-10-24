import glob
import os
from typing import Any, Dict, List

import requests


class GraphragApiLibrary:
    """Robot Framework keyword library for interacting with the running Graphrag API and HaMS sidecar."""

    ROBOT_LIBRARY_SCOPE = "SUITE"

    def __init__(
        self,
        be_base_url: str = "http://localhost:8080",
        hams_base_url: str = "http://localhost:8079",
    ):
        self.be_base_url = be_base_url.rstrip("/")
        self.hams_base_url = hams_base_url.rstrip("/")
        self.ingested_document_ids: List[str] = []

    def verify_hams_liveness(self) -> None:
        """Verifies that the HaMS /hams/alive endpoint returns HTTP 200."""
        url = f"{self.hams_base_url}/hams/alive"
        resp = requests.get(url, timeout=10)
        if resp.status_code != 200:
            raise AssertionError(
                f"HaMS liveness check failed at {url}: status={resp.status_code}, body={resp.text}"
            )

    def verify_hams_readiness(self) -> None:
        """Verifies that the HaMS /hams/ready endpoint returns HTTP 200."""
        url = f"{self.hams_base_url}/hams/ready"
        resp = requests.get(url, timeout=10)
        if resp.status_code != 200:
            raise AssertionError(
                f"HaMS readiness check failed at {url}: status={resp.status_code}, body={resp.text}"
            )

    def verify_hams_metrics(self) -> str:
        """Verifies that the HaMS /hams/metrics endpoint returns non-empty Prometheus metrics."""
        url = f"{self.hams_base_url}/hams/metrics"
        resp = requests.get(url, timeout=10)
        if resp.status_code != 200 or not resp.text.strip():
            raise AssertionError(
                f"HaMS metrics check failed at {url}: status={resp.status_code}"
            )
        return resp.text

    def get_hello_endpoint(self) -> str:
        """Calls GET /hello on the running Graphrag Axum service."""
        url = f"{self.be_base_url}/hello"
        resp = requests.get(url, timeout=10)
        if resp.status_code != 200:
            raise AssertionError(
                f"GET /hello failed at {url}: status={resp.status_code}, body={resp.text}"
            )
        return resp.text

    def load_markdown_file(self, file_path: str) -> Dict[str, Any]:
        """Reads a markdown file from disk and ingests it via POST /ingest on the Graphrag API."""
        if not os.path.isfile(file_path):
            raise AssertionError(f"Markdown file not found: {file_path}")

        with open(file_path, "r", encoding="utf-8") as f:
            content = f.read()

        filename = os.path.basename(file_path)
        url = f"{self.be_base_url}/ingest"
        payload = {"filename": filename, "content": content}
        resp = requests.post(url, json=payload, timeout=120)
        if resp.status_code not in (200, 201):
            raise AssertionError(
                f"Failed to ingest markdown file {filename} at {url}: status={resp.status_code}, body={resp.text}"
            )

        data = resp.json()
        doc_id = data.get("document_id") or data.get("id") or filename
        self.ingested_document_ids.append(str(doc_id))
        return data

    def load_baseline_markdown_directory(self, directory_path: str) -> List[Dict[str, Any]]:
        """Loads all .md files in the given directory (sorted alphabetically) into Graphrag."""
        pattern = os.path.join(directory_path, "*.md")
        md_files = sorted(glob.glob(pattern))
        if not md_files:
            raise AssertionError(f"No .md files found in directory: {directory_path}")

        results = []
        for path in md_files:
            results.append(self.load_markdown_file(path))
        return results

    def ask_graphrag_question(self, question: str) -> Dict[str, Any]:
        """Sends a natural-language question to POST /query and returns {'answer': ..., 'contexts': [...]}."""
        url = f"{self.be_base_url}/query"
        resp = requests.post(url, json={"question": question}, timeout=120)
        if resp.status_code != 200:
            raise AssertionError(
                f"POST /query failed at {url}: status={resp.status_code}, body={resp.text}"
            )
        data = resp.json()
        if "answer" not in data:
            raise AssertionError(f"Response from /query missing 'answer' field: {data}")
        if "contexts" not in data:
            data["contexts"] = []
        return data

    def cleanup_ingested_documents(self) -> None:
        """Deletes all documents ingested during the suite in [Teardown]."""
        while self.ingested_document_ids:
            doc_id = self.ingested_document_ids.pop()
            url = f"{self.be_base_url}/documents/{doc_id}"
            try:
                requests.delete(url, timeout=15)
            except requests.RequestException:
                pass

    # =========================================================================
    # BREAD API Methods: Documents
    # =========================================================================

    def browse_documents(
        self,
        query: str = None,
        limit: int = 50,
        offset: int = 0,
        expected_status: int = 200,
    ) -> Dict[str, Any]:
        """Calls GET /documents."""
        params = {"limit": limit, "offset": offset}
        if query:
            params["q"] = query
        resp = requests.get(f"{self.be_base_url}/documents", params=params, timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"GET /documents expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json()

    def read_document(self, doc_id: str, expected_status: int = 200) -> Dict[str, Any]:
        """Calls GET /documents/{id}."""
        resp = requests.get(f"{self.be_base_url}/documents/{doc_id}", timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"GET /documents/{doc_id} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json()

    def edit_document(
        self,
        doc_id: str,
        content: str = None,
        filename: str = None,
        re_extract: bool = True,
        expected_status: int = 200,
    ) -> Any:
        """Calls PUT /documents/{id}."""
        payload = {"re_extract": re_extract}
        if content is not None:
            payload["content"] = content
        if filename is not None:
            payload["filename"] = filename
        resp = requests.put(f"{self.be_base_url}/documents/{doc_id}", json=payload, timeout=60)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"PUT /documents/{doc_id} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json() if resp.text else None

    def add_document(self, filename: str, content: str, expected_status: int = 201) -> Dict[str, Any]:
        """Calls POST /documents."""
        payload = {"filename": filename, "content": content}
        resp = requests.post(f"{self.be_base_url}/documents", json=payload, timeout=60)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"POST /documents expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        data = resp.json()
        doc_id = data.get("document_id") or filename
        self.ingested_document_ids.append(str(doc_id))
        return data

    def delete_document(self, doc_id: str, expected_status: int = 204) -> None:
        """Calls DELETE /documents/{id}."""
        resp = requests.delete(f"{self.be_base_url}/documents/{doc_id}", timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"DELETE /documents/{doc_id} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )

    # =========================================================================
    # BREAD API Methods: Nodes (Knowledge Objects)
    # =========================================================================

    def browse_nodes(
        self,
        label: str = None,
        layer: int = None,
        document_id: str = None,
        query: str = None,
        semantic_query: str = None,
        limit: int = 50,
        offset: int = 0,
        expected_status: int = 200,
    ) -> Dict[str, Any]:
        """Calls GET /nodes."""
        params = {"limit": limit, "offset": offset}
        if label:
            params["label"] = label
        if layer is not None:
            params["layer"] = layer
        if document_id:
            params["document_id"] = document_id
        if query:
            params["q"] = query
        if semantic_query:
            params["semantic_query"] = semantic_query
        resp = requests.get(f"{self.be_base_url}/nodes", params=params, timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"GET /nodes expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json()

    def read_node(self, node_id: str, expected_status: int = 200) -> Dict[str, Any]:
        """Calls GET /nodes/{id}."""
        resp = requests.get(f"{self.be_base_url}/nodes/{node_id}", timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"GET /nodes/{node_id} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json()

    def add_node(
        self,
        node_id: str,
        label: str,
        description: str,
        layer: int = 0,
        document_id: str = None,
        expected_status: int = 201,
    ) -> Any:
        """Calls POST /nodes."""
        payload = {
            "id": node_id,
            "label": label,
            "description": description,
            "layer": layer,
            "document_id": document_id,
            "embedding": [],
        }
        resp = requests.post(f"{self.be_base_url}/nodes", json=payload, timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"POST /nodes expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json() if resp.text else None

    def edit_node(
        self,
        node_id: str,
        label: str = None,
        description: str = None,
        layer: int = None,
        document_id: str = None,
        expected_status: int = 200,
    ) -> Any:
        """Calls PUT /nodes/{id}."""
        payload = {}
        if label is not None:
            payload["label"] = label
        if description is not None:
            payload["description"] = description
        if layer is not None:
            payload["layer"] = layer
        if document_id is not None:
            payload["document_id"] = document_id
        resp = requests.put(f"{self.be_base_url}/nodes/{node_id}", json=payload, timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"PUT /nodes/{node_id} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json() if resp.text else None

    def delete_node(self, node_id: str, expected_status: int = 204) -> None:
        """Calls DELETE /nodes/{id}."""
        resp = requests.delete(f"{self.be_base_url}/nodes/{node_id}", timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"DELETE /nodes/{node_id} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )

    # =========================================================================
    # BREAD API Methods: Edges (Graph Tuples)
    # =========================================================================

    def browse_edges(
        self,
        source: str = None,
        predicate: str = None,
        target: str = None,
        document_id: str = None,
        limit: int = 50,
        offset: int = 0,
        expected_status: int = 200,
    ) -> Dict[str, Any]:
        """Calls GET /edges."""
        params = {"limit": limit, "offset": offset}
        if source:
            params["source"] = source
        if predicate:
            params["predicate"] = predicate
        if target:
            params["target"] = target
        if document_id:
            params["document_id"] = document_id
        resp = requests.get(f"{self.be_base_url}/edges", params=params, timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"GET /edges expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json()

    def read_edge(
        self, source: str, predicate: str, target: str, expected_status: int = 200
    ) -> Dict[str, Any]:
        """Calls GET /edges/{source}/{predicate}/{target}."""
        resp = requests.get(
            f"{self.be_base_url}/edges/{source}/{predicate}/{target}", timeout=15
        )
        if resp.status_code != expected_status:
            raise AssertionError(
                f"GET /edges/{source}/{predicate}/{target} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json()

    def add_edge(
        self,
        source: str,
        predicate: str,
        target: str,
        document_id: str = None,
        auto_create_nodes: bool = False,
        expected_status: int = 201,
    ) -> Any:
        """Calls POST /edges."""
        payload = {
            "source": source,
            "predicate": predicate,
            "target": target,
            "document_id": document_id,
            "auto_create_nodes": auto_create_nodes,
        }
        resp = requests.post(f"{self.be_base_url}/edges", json=payload, timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"POST /edges expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json() if resp.text else None

    def edit_edge(
        self,
        source: str,
        predicate: str,
        target: str,
        new_predicate: str = None,
        new_target: str = None,
        document_id: str = None,
        expected_status: int = 200,
    ) -> Any:
        """Calls PUT /edges/{source}/{predicate}/{target}."""
        payload = {}
        if new_predicate is not None:
            payload["predicate"] = new_predicate
        if new_target is not None:
            payload["target"] = new_target
        if document_id is not None:
            payload["document_id"] = document_id
        resp = requests.put(
            f"{self.be_base_url}/edges/{source}/{predicate}/{target}",
            json=payload,
            timeout=15,
        )
        if resp.status_code != expected_status:
            raise AssertionError(
                f"PUT /edges/{source}/{predicate}/{target} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json() if resp.text else None

    def delete_edge(
        self, source: str, predicate: str, target: str, expected_status: int = 204
    ) -> None:
        """Calls DELETE /edges/{source}/{predicate}/{target}."""
        resp = requests.delete(
            f"{self.be_base_url}/edges/{source}/{predicate}/{target}", timeout=15
        )
        if resp.status_code != expected_status:
            raise AssertionError(
                f"DELETE /edges expected {expected_status}, got {resp.status_code}: {resp.text}"
            )

    # =========================================================================
    # BREAD API Methods: Communities
    # =========================================================================

    def browse_communities(
        self,
        query: str = None,
        member: str = None,
        limit: int = 50,
        offset: int = 0,
        expected_status: int = 200,
    ) -> Dict[str, Any]:
        """Calls GET /communities."""
        params = {"limit": limit, "offset": offset}
        if query:
            params["q"] = query
        if member:
            params["member"] = member
        resp = requests.get(f"{self.be_base_url}/communities", params=params, timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"GET /communities expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json()

    def read_community(self, comm_id: str, expected_status: int = 200) -> Dict[str, Any]:
        """Calls GET /communities/{id}."""
        resp = requests.get(f"{self.be_base_url}/communities/{comm_id}", timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"GET /communities/{comm_id} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json()

    def edit_community(
        self, comm_id: str, description: str, expected_status: int = 200
    ) -> Any:
        """Calls PUT /communities/{id}."""
        payload = {"description": description}
        resp = requests.put(
            f"{self.be_base_url}/communities/{comm_id}", json=payload, timeout=15
        )
        if resp.status_code != expected_status:
            raise AssertionError(
                f"PUT /communities/{comm_id} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json() if resp.text else None

    def rebuild_communities(self, expected_status: int = 200) -> Any:
        """Calls POST /communities/rebuild."""
        resp = requests.post(f"{self.be_base_url}/communities/rebuild", timeout=30)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"POST /communities/rebuild expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        return resp.json() if resp.text else None

    def delete_community(self, comm_id: str, expected_status: int = 204) -> None:
        """Calls DELETE /communities/{id}."""
        resp = requests.delete(f"{self.be_base_url}/communities/{comm_id}", timeout=15)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"DELETE /communities/{comm_id} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )

    def mcp_request(
        self, payload: Dict[str, Any], endpoint: str = "/api/v1/mcp", expected_status: int = 200
    ) -> Any:
        """Sends a JSON-RPC 2.0 request payload to an MCP endpoint (/api/v1/mcp or /mcp)."""
        url = f"{self.be_base_url}{endpoint}"
        resp = requests.post(url, json=payload, timeout=30)
        if resp.status_code != expected_status:
            raise AssertionError(
                f"POST {endpoint} expected {expected_status}, got {resp.status_code}: {resp.text}"
            )
        if resp.status_code == 204 or not resp.text.strip():
            return None
        return resp.json()

    def mcp_initialize(self, endpoint: str = "/api/v1/mcp") -> Dict[str, Any]:
        """Performs MCP initialize handshake."""
        payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "robot-test-client", "version": "1.0.0"},
            },
        }
        resp = self.mcp_request(payload, endpoint=endpoint)
        if "error" in resp:
            raise AssertionError(f"MCP initialize returned error: {resp['error']}")
        return resp.get("result", {})

    def mcp_notify_initialized(self, endpoint: str = "/api/v1/mcp") -> None:
        """Sends notifications/initialized notification (204 No Content expected)."""
        payload = {
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        }
        self.mcp_request(payload, endpoint=endpoint, expected_status=204)

    def mcp_ping(self, endpoint: str = "/api/v1/mcp") -> Dict[str, Any]:
        """Sends ping method to MCP server."""
        payload = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "ping",
        }
        resp = self.mcp_request(payload, endpoint=endpoint)
        return resp.get("result", {})

    def mcp_list_tools(self, endpoint: str = "/api/v1/mcp") -> List[Dict[str, Any]]:
        """Queries tools/list on MCP server."""
        payload = {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/list",
        }
        resp = self.mcp_request(payload, endpoint=endpoint)
        return resp.get("result", {}).get("tools", [])

    def mcp_call_tool(
        self,
        tool_name: str,
        arguments: Dict[str, Any] = None,
        endpoint: str = "/api/v1/mcp",
    ) -> Dict[str, Any]:
        """Calls a tool via tools/call and returns the tool call result object."""
        if arguments is None:
            arguments = {}
        payload = {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments,
            },
        }
        resp = self.mcp_request(payload, endpoint=endpoint)
        return resp.get("result", {})
