import asyncio
import os
from typing import Any, Callable, Dict, List, Optional


class RagasEvalLibrary:
    """Robot Framework keyword library wrapping Ragas metrics to evaluate Graphrag AI responses."""

    ROBOT_LIBRARY_SCOPE = "GLOBAL"

    def __init__(
        self,
        llm_base_url: Optional[str] = None,
        llm_model: Optional[str] = None,
        llm_api_key: Optional[str] = None,
        embedding_model: Optional[str] = None,
        scorer: Optional[
            Callable[[str, str, List[str], str], Dict[str, float]]
        ] = None,
    ):
        self.llm_base_url = llm_base_url or os.environ.get(
            "LLM_BASE_URL", "http://localhost:11434/v1"
        )
        self.llm_model = llm_model or os.environ.get("LLM_MODEL", "llama3")
        self.llm_api_key = llm_api_key or os.environ.get(
            "LLM_API_KEY", "sk-no-key-required"
        )
        self.embedding_model = embedding_model or os.environ.get(
            "LLM_EMBEDDING_MODEL", self.llm_model
        )
        self._scorer = scorer

    def _run_ragas_single_turn(
        self,
        question: str,
        actual_answer: str,
        retrieved_contexts: List[str],
        reference_answer: str,
    ) -> Dict[str, float]:
        """Executes Ragas SingleTurnSample metrics against the configured OpenAI-compatible LLM."""
        if self._scorer is not None:
            return self._scorer(
                question, actual_answer, retrieved_contexts, reference_answer
            )

        from langchain_openai import ChatOpenAI, OpenAIEmbeddings
        from ragas.dataset_schema import SingleTurnSample
        from ragas.embeddings import LangchainEmbeddingsWrapper
        from ragas.llms import LangchainLLMWrapper
        from ragas.metrics import (
            FactualCorrectness,
            Faithfulness,
            LLMContextRecall,
            ResponseRelevancy,
        )

        chat_llm = ChatOpenAI(
            model=self.llm_model,
            base_url=self.llm_base_url,
            api_key=self.llm_api_key,
            temperature=0.0,
        )
        embeddings = OpenAIEmbeddings(
            model=self.embedding_model,
            base_url=self.llm_base_url,
            api_key=self.llm_api_key,
        )

        evaluator_llm = LangchainLLMWrapper(chat_llm)
        evaluator_embeddings = LangchainEmbeddingsWrapper(embeddings)

        sample = SingleTurnSample(
            user_input=question,
            response=actual_answer,
            retrieved_contexts=list(retrieved_contexts),
            reference=reference_answer,
        )

        faithfulness_metric = Faithfulness(llm=evaluator_llm)
        factual_metric = FactualCorrectness(llm=evaluator_llm)
        relevancy_metric = ResponseRelevancy(
            llm=evaluator_llm, embeddings=evaluator_embeddings
        )
        recall_metric = LLMContextRecall(llm=evaluator_llm)

        async def _score_all() -> Dict[str, float]:
            faithfulness_score = await faithfulness_metric.single_turn_ascore(sample)
            factual_score = await factual_metric.single_turn_ascore(sample)
            relevancy_score = await relevancy_metric.single_turn_ascore(sample)
            recall_score = await recall_metric.single_turn_ascore(sample)
            return {
                "faithfulness": float(faithfulness_score),
                "factual_correctness": float(factual_score),
                "response_relevancy": float(relevancy_score),
                "context_recall": float(recall_score),
            }

        return asyncio.run(_score_all())

    def evaluate_rag_response_with_ragas(
        self,
        question: str,
        actual_answer: str,
        retrieved_contexts: List[str],
        reference_answer: str,
        min_faithfulness: float = 0.70,
        min_factual_correctness: float = 0.70,
    ) -> Dict[str, float]:
        """
        Grades an AI response from Graphrag using Ragas metrics and asserts minimum thresholds.

        Logs all computed Ragas metric scores into Robot Framework's log.html.
        """
        min_faithfulness = float(min_faithfulness)
        min_factual_correctness = float(min_factual_correctness)

        scores = self._run_ragas_single_turn(
            question=question,
            actual_answer=actual_answer,
            retrieved_contexts=retrieved_contexts,
            reference_answer=reference_answer,
        )

        print(f"Ragas Evaluation Scores for question='{question}': {scores}")

        failures = []
        faithfulness = scores.get("faithfulness", 0.0)
        factual_correctness = scores.get("factual_correctness", 0.0)

        if faithfulness < min_faithfulness:
            failures.append(
                f"faithfulness={faithfulness:.3f} < threshold {min_faithfulness:.3f}"
            )
        if factual_correctness < min_factual_correctness:
            failures.append(
                f"factual_correctness={factual_correctness:.3f} < threshold {min_factual_correctness:.3f}"
            )

        if failures:
            raise AssertionError(
                f"Ragas evaluation failed ({', '.join(failures)}). "
                f"Actual answer: '{actual_answer}' | Reference: '{reference_answer}' | All scores: {scores}"
            )

        return scores
