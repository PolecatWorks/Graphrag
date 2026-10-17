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
