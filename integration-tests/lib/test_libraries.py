import os
import tempfile
import unittest
from unittest.mock import MagicMock, patch

from GraphragApiLibrary import GraphragApiLibrary
from RagasEvalLibrary import RagasEvalLibrary


class TestGraphragApiLibrary(unittest.TestCase):
    def setUp(self):
        self.lib = GraphragApiLibrary(
            be_base_url="http://localhost:8080",
            hams_base_url="http://localhost:8079",
        )

    @patch("GraphragApiLibrary.requests.get")
    def test_verify_hams_liveness(self, mock_get):
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_get.return_value = mock_resp

        self.lib.verify_hams_liveness()
        mock_get.assert_called_once_with("http://localhost:8079/hams/alive", timeout=10)

    @patch("GraphragApiLibrary.requests.get")
    def test_verify_hams_readiness_failure(self, mock_get):
        mock_resp = MagicMock()
        mock_resp.status_code = 503
        mock_resp.text = "Not ready"
        mock_get.return_value = mock_resp

        with self.assertRaises(AssertionError):
            self.lib.verify_hams_readiness()

    @patch("GraphragApiLibrary.requests.get")
    def test_get_hello_endpoint(self, mock_get):
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.text = "Hello, World!"
        mock_get.return_value = mock_resp

        text = self.lib.get_hello_endpoint()
        self.assertEqual(text, "Hello, World!")
        mock_get.assert_called_once_with("http://localhost:8080/hello", timeout=10)

    @patch("GraphragApiLibrary.requests.post")
    def test_load_markdown_file_and_cleanup(self, mock_post):
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.json.return_value = {"document_id": "doc-123", "status": "ingested"}
        mock_post.return_value = mock_resp

        with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False) as f:
            f.write("# Test Doc\nHello Apollo 11.")
            temp_path = f.name

        try:
            res = self.lib.load_markdown_file(temp_path)
            self.assertEqual(res["document_id"], "doc-123")
            self.assertIn("doc-123", self.lib.ingested_document_ids)
        finally:
            os.remove(temp_path)

    @patch("GraphragApiLibrary.requests.post")
    def test_ask_graphrag_question(self, mock_post):
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.json.return_value = {
            "answer": "Michael Collins piloted the Command Module Columbia.",
            "contexts": [
                "Michael Collins flew the Apollo 11 command module Columbia around the Moon in 1969."
            ],
        }
        mock_post.return_value = mock_resp

        result = self.lib.ask_graphrag_question("Who piloted the Command Module?")
        self.assertEqual(
            result["answer"], "Michael Collins piloted the Command Module Columbia."
        )
        self.assertEqual(len(result["contexts"]), 1)


class TestRagasEvalLibrary(unittest.TestCase):
    def test_evaluate_rag_response_with_injected_scorer(self):
        def fake_scorer(question, actual_answer, retrieved_contexts, reference_answer):
            return {
                "faithfulness": 0.92,
                "factual_correctness": 0.88,
                "response_relevancy": 0.95,
            }

        lib = RagasEvalLibrary(
            llm_base_url="http://localhost:11434/v1",
            llm_model="llama3",
            scorer=fake_scorer,
        )

        scores = lib.evaluate_rag_response_with_ragas(
            question="Where was Michael Collins born?",
            actual_answer="Michael Collins was born in Rome, Italy.",
            retrieved_contexts=["Born in Rome, Italy, Collins graduated from West Point."],
            reference_answer="Michael Collins was born in Rome, Italy.",
            min_faithfulness=0.75,
            min_factual_correctness=0.75,
        )
        self.assertGreaterEqual(scores["faithfulness"], 0.75)
        self.assertGreaterEqual(scores["factual_correctness"], 0.75)

    def test_evaluate_rag_response_fails_below_threshold(self):
        def low_scorer(question, actual_answer, retrieved_contexts, reference_answer):
            return {
                "faithfulness": 0.40,
                "factual_correctness": 0.85,
            }

        lib = RagasEvalLibrary(
            llm_base_url="http://localhost:11434/v1",
            llm_model="llama3",
            scorer=low_scorer,
        )

        with self.assertRaises(AssertionError) as ctx:
            lib.evaluate_rag_response_with_ragas(
                question="Where was Michael Collins born?",
                actual_answer="He was born in Paris, France.",
                retrieved_contexts=["Born in Rome, Italy, Collins graduated from West Point."],
                reference_answer="Michael Collins was born in Rome, Italy.",
                min_faithfulness=0.75,
                min_factual_correctness=0.75,
            )
        self.assertIn("faithfulness", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
