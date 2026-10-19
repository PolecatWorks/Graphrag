*** Settings ***
Documentation     Loads the baseline Apollo 11 markdown dataset into the running Graphrag API, executes single-hop and multi-hop queries, and evaluates AI responses using Ragas.
Library           ../lib/GraphragApiLibrary.py    be_base_url=${BE_BASE_URL}    hams_base_url=${HAMS_BE_URL}
Library           ../lib/RagasEvalLibrary.py      llm_base_url=${LLM_BASE_URL}    llm_model=${LLM_MODEL}
Suite Setup       Load Baseline Markdown Directory    ${BASELINE_DATA_DIR}
Suite Teardown    Cleanup Ingested Documents

*** Variables ***
${BE_BASE_URL}          http://localhost:8080
${HAMS_BE_URL}          http://localhost:8079
${LLM_BASE_URL}         http://localhost:11434/v1
${LLM_MODEL}            llama3.1:8b
${BASELINE_DATA_DIR}    ${CURDIR}/../../tests/data/graph_rag_baseline

*** Test Cases ***
Single-Hop Query: Apollo 11 Command Module Pilot
    [Documentation]    Verifies Graphrag accurately answers who piloted the Command Module Columbia and remained in lunar orbit.
    [Tags]    graphrag    single-hop    ragas
    ${question}=    Set Variable    Who flew the Apollo 11 command module Columbia in lunar orbit while Neil Armstrong and Buzz Aldrin landed on the Moon?
    ${reference}=   Set Variable    Michael Collins flew the Apollo 11 command module Columbia in lunar orbit while Neil Armstrong and Buzz Aldrin landed on the Moon.
    ${result}=      Ask Graphrag Question    ${question}
    Evaluate Rag Response With Ragas
    ...    question=${question}
    ...    actual_answer=${result}[answer]
    ...    retrieved_contexts=${result}[contexts]
    ...    reference_answer=${reference}
    ...    min_faithfulness=0.70
    ...    min_factual_correctness=0.70

Multi-Hop Query: Presidential Goal Fulfilled By Apollo 11 Crew
    [Documentation]    Traverses across Project Apollo, JFK, and the Apollo 11 crew to verify multi-hop synthesis.
    [Tags]    graphrag    multi-hop    ragas
    ${question}=    Set Variable    Which US President proposed the national goal in May 1961 that was accomplished on July 20, 1969 by Neil Armstrong and Buzz Aldrin, and what university did the Lunar Module Eagle pilot earn his doctorate from?
    ${reference}=   Set Variable    President John F. Kennedy proposed the national goal in May 1961 that was accomplished on July 20, 1969 by Neil Armstrong and Buzz Aldrin, and the Lunar Module Eagle pilot Buzz Aldrin earned his Doctor of Science doctorate from the Massachusetts Institute of Technology (MIT).
    ${result}=      Ask Graphrag Question    ${question}
    Evaluate Rag Response With Ragas
    ...    question=${question}
    ...    actual_answer=${result}[answer]
    ...    retrieved_contexts=${result}[contexts]
    ...    reference_answer=${reference}
    ...    min_faithfulness=0.70
    ...    min_factual_correctness=0.70

Multi-Hop Query: West Point Graduates On Apollo 11
    [Documentation]    Synthesizes biographical facts across Buzz Aldrin and Michael Collins markdown documents.
    [Tags]    graphrag    multi-hop    ragas
    ${question}=    Set Variable    Which Apollo 11 astronauts graduated from the United States Military Academy at West Point, and in what years did they graduate?
    ${reference}=   Set Variable    Buzz Aldrin and Michael Collins both graduated from the United States Military Academy at West Point; Buzz Aldrin graduated in 1951 and Michael Collins graduated in 1952.
    ${result}=      Ask Graphrag Question    ${question}
    Evaluate Rag Response With Ragas
    ...    question=${question}
    ...    actual_answer=${result}[answer]
    ...    retrieved_contexts=${result}[contexts]
    ...    reference_answer=${reference}
    ...    min_faithfulness=0.70
    ...    min_factual_correctness=0.70
