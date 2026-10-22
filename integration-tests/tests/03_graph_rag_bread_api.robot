*** Settings ***
Documentation     End-to-End integration test suite for Graph RAG Browse, Read, Edit, Add, Delete (BREAD) REST APIs.
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
BREAD Operations: Documents Resource
    [Documentation]    Exercises Browse, Read, Edit, Add, Delete across /documents endpoints.
    [Tags]    graphrag    bread    documents
    # Browse documents
    ${docs}=    Browse Documents    limit=10
    Should Be True    ${docs}[total] >= 5

    # Read specific document
    ${doc_resp}=    Read Document    04_michael_collins.md
    Should Be Equal As Strings    ${doc_resp}[document][id]    04_michael_collins.md
    Should Contain    ${doc_resp}[document][content]    Michael Collins

    # Add a temporary document
    ${add_resp}=    Add Document    filename=temp_apollo_bread.md    content=# Apollo Bread Test\n\nApollo Bread Test verifies document BREAD.
    Should Be Equal As Strings    ${add_resp}[document_id]    temp_apollo_bread.md

    # Edit the document
    Edit Document    doc_id=temp_apollo_bread.md    content=# Apollo Bread Test Updated\n\nApollo Bread Test updated content.
    ${read_updated}=    Read Document    temp_apollo_bread.md
    Should Contain    ${read_updated}[document][content]    updated content

    # Delete the document
    Delete Document    temp_apollo_bread.md
    Read Document    temp_apollo_bread.md    expected_status=404

BREAD Operations: Nodes Resource
    [Documentation]    Exercises Browse, Read, Edit, Add, Delete across /nodes endpoints.
    [Tags]    graphrag    bread    nodes
    # Browse nodes
    ${nodes}=    Browse Nodes    limit=10
    Should Be True    ${nodes}[total] >= 1

    # Read specific node
    ${node}=    Read Node    michael_collins
    Should Be Equal As Strings    ${node}[node][id]    michael_collins

    # Add custom node
    Add Node    node_id=gene_kranz    label=FlightDirector    description=Gene Kranz was the Apollo 11 lead flight director in Mission Control.
    ${new_node}=    Read Node    gene_kranz
    Should Be Equal As Strings    ${new_node}[node][label]    FlightDirector

    # Edit node
    Edit Node    node_id=gene_kranz    description=Gene Kranz served as NASA Flight Director for Gemini and Apollo missions.
    ${edited_node}=    Read Node    gene_kranz
    Should Contain    ${edited_node}[node][description]    Gemini and Apollo missions

    # Delete node
    Delete Node    gene_kranz
    Read Node    gene_kranz    expected_status=404

BREAD Operations: Edges Resource
    [Documentation]    Exercises Browse, Read, Edit, Add, Delete across /edges endpoints.
    [Tags]    graphrag    bread    edges
    # Browse edges
    ${edges}=    Browse Edges    limit=10
    Should Be True    ${edges}[total] >= 1

    # Add custom edge connecting existing nodes
    Add Edge    source=michael_collins    predicate=PILOTED_MISSION    target=apollo_11
    ${edge}=    Read Edge    source=michael_collins    predicate=PILOTED_MISSION    target=apollo_11
    Should Be Equal As Strings    ${edge}[edge][predicate]    PILOTED_MISSION

    # Edit edge predicate
    Edit Edge    source=michael_collins    predicate=PILOTED_MISSION    target=apollo_11    new_predicate=COMMAND_MODULE_PILOT_FOR
    Read Edge    source=michael_collins    predicate=PILOTED_MISSION    target=apollo_11    expected_status=404
    ${updated_edge}=    Read Edge    source=michael_collins    predicate=COMMAND_MODULE_PILOT_FOR    target=apollo_11
    Should Be Equal As Strings    ${updated_edge}[edge][predicate]    COMMAND_MODULE_PILOT_FOR

    # Delete edge
    Delete Edge    source=michael_collins    predicate=COMMAND_MODULE_PILOT_FOR    target=apollo_11
    Read Edge    source=michael_collins    predicate=COMMAND_MODULE_PILOT_FOR    target=apollo_11    expected_status=404

BREAD Operations: Communities Resource
    [Documentation]    Exercises Browse, Read, Edit, and Rebuild across /communities endpoints.
    [Tags]    graphrag    bread    communities
    # Rebuild communities
    ${rebuilt}=    Rebuild Communities
    Should Not Be Empty    ${rebuilt}

    # Browse communities
    ${comms}=    Browse Communities    limit=10
    Should Be True    ${comms}[total] >= 1
    ${first_id}=    Set Variable    ${comms}[items][0][id]

    # Read community
    ${comm}=    Read Community    ${first_id}
    Should Be Equal As Strings    ${comm}[community][id]    ${first_id}

    # Edit community description
    Edit Community    comm_id=${first_id}    description=Updated community description for Apollo 11 graph.
    ${updated_comm}=    Read Community    ${first_id}
    Should Contain    ${updated_comm}[community][description]    Updated community description

BREAD Impact On Graphrag Query Answering
    [Documentation]    Verifies that adding a new node and edge via BREAD immediately reflects in subsequent /query answers.
    [Tags]    graphrag    bread    query
    # Add an astronaut and relationship
    Add Node    node_id=fred_haise    label=Astronaut    description=Fred Haise served as Lunar Module Pilot on Apollo 13.
    Add Edge    source=fred_haise    predicate=ASSIGNED_TO    target=apollo_13    auto_create_nodes=True

    # Ask question
    ${question}=    Set Variable    Which mission was astronaut Fred Haise assigned to as Lunar Module Pilot?
    ${result}=      Ask Graphrag Question    ${question}
    Should Contain    ${result}[answer]    Apollo 13
    Should Be True    any("fred_haise" in ctx for ctx in ${result}[contexts])

    # Clean up
    Delete Edge    fred_haise    ASSIGNED_TO    apollo_13
    Delete Node    fred_haise
