*** Settings ***
Documentation     End-to-End integration test suite for Graph RAG Model Context Protocol (MCP) Service.
Library           ../lib/GraphragApiLibrary.py    be_base_url=${BE_BASE_URL}    hams_base_url=${HAMS_BE_URL}
Library           Collections
Suite Setup       Load Baseline Markdown Directory    ${BASELINE_DATA_DIR}
Suite Teardown    Cleanup Ingested Documents

*** Variables ***
${BE_BASE_URL}          http://localhost:8080
${HAMS_BE_URL}          http://localhost:8079
${BASELINE_DATA_DIR}    ${CURDIR}/../data/graph_rag_baseline

*** Test Cases ***
MCP Protocol Handshake and Discovery
    [Documentation]    Verifies initialize, ping, notifications/initialized, and tools/list via JSON-RPC 2.0.
    [Tags]    mcp    handshake

    # 1. Initialize
    ${init_resp}=    Mcp Initialize    endpoint=/api/v1/mcp
    Should Be Equal As Strings    ${init_resp}[protocolVersion]    2024-11-05
    Should Be Equal As Strings    ${init_resp}[serverInfo][name]    graphrag-mcp

    # 2. Notifications Initialized (204 No Content)
    Mcp Notify Initialized    endpoint=/api/v1/mcp

    # 3. Ping
    ${ping_resp}=    Mcp Ping    endpoint=/api/v1/mcp
    Should Be Empty    ${ping_resp}

    # 4. Tools List
    ${tools}=    Mcp List Tools    endpoint=/api/v1/mcp
    Length Should Be    ${tools}    23

    # Verify key tool schemas exist
    ${names}=    Create List
    FOR    ${t}    IN    @{tools}
        Append To List    ${names}    ${t}[name]
    END
    List Should Contain Value    ${names}    hello
    List Should Contain Value    ${names}    graphrag_ingest
    List Should Contain Value    ${names}    graphrag_query
    List Should Contain Value    ${names}    document_browse
    List Should Contain Value    ${names}    document_read
    List Should Contain Value    ${names}    document_edit
    List Should Contain Value    ${names}    document_add
    List Should Contain Value    ${names}    document_delete
    List Should Contain Value    ${names}    node_browse
    List Should Contain Value    ${names}    node_read
    List Should Contain Value    ${names}    node_edit
    List Should Contain Value    ${names}    node_add
    List Should Contain Value    ${names}    node_delete
    List Should Contain Value    ${names}    edge_browse
    List Should Contain Value    ${names}    edge_read
    List Should Contain Value    ${names}    edge_edit
    List Should Contain Value    ${names}    edge_add
    List Should Contain Value    ${names}    edge_delete
    List Should Contain Value    ${names}    community_browse
    List Should Contain Value    ${names}    community_read
    List Should Contain Value    ${names}    community_edit
    List Should Contain Value    ${names}    community_rebuild
    List Should Contain Value    ${names}    community_delete

MCP Baseline Tool: Hello
    [Documentation]    Verifies canonical hello smoke-test tool with greeting and error behavior.
    [Tags]    mcp    hello

    # Valid invocation with whitespace trimming
    ${args}=    Create Dictionary    name=${SPACE}${SPACE}Antigravity Agent${SPACE}${SPACE}
    ${res}=    Mcp Call Tool    tool_name=hello    arguments=${args}
    Should Be Equal    ${res}[isError]    ${False}
    Should Be Equal As Strings    ${res}[content][0][text]    Hello, Antigravity Agent!

    # Empty name should return isError: true
    ${empty_args}=    Create Dictionary    name=${SPACE}${SPACE}
    ${err_res}=    Mcp Call Tool    tool_name=hello    arguments=${empty_args}
    Should Be Equal    ${err_res}[isError]    ${True}

MCP BREAD Lifecycle: Documents
    [Documentation]    Exercises document BREAD tools over MCP JSON-RPC.
    [Tags]    mcp    bread    documents

    # 1. document_browse
    ${limit_int}=    Convert To Integer    10
    ${browse_args}=    Create Dictionary    limit=${limit_int}
    ${browse_res}=    Mcp Call Tool    tool_name=document_browse    arguments=${browse_args}
    Should Be Equal    ${browse_res}[isError]    ${False}
    Should Contain    ${browse_res}[content][0][text]    04_michael_collins.md

    # 2. document_read
    ${read_args}=    Create Dictionary    id=04_michael_collins.md
    ${read_res}=    Mcp Call Tool    tool_name=document_read    arguments=${read_args}
    Should Be Equal    ${read_res}[isError]    ${False}
    Should Contain    ${read_res}[content][0][text]    Michael Collins

    # 3. document_add
    ${add_args}=    Create Dictionary    filename=temp_mcp_doc.md    content=# MCP Integration\n\nMCP document content test.
    ${add_res}=    Mcp Call Tool    tool_name=document_add    arguments=${add_args}
    Should Be Equal    ${add_res}[isError]    ${False}
    Should Contain    ${add_res}[content][0][text]    temp_mcp_doc.md

    # 4. document_edit
    ${edit_args}=    Create Dictionary    id=temp_mcp_doc.md    content=# MCP Integration Updated\n\nUpdated document content test.
    ${edit_res}=    Mcp Call Tool    tool_name=document_edit    arguments=${edit_args}
    Should Be Equal    ${edit_res}[isError]    ${False}
    Should Contain    ${edit_res}[content][0][text]    Updated document content

    # 5. document_delete
    ${del_args}=    Create Dictionary    id=temp_mcp_doc.md
    ${del_res}=    Mcp Call Tool    tool_name=document_delete    arguments=${del_args}
    Should Be Equal    ${del_res}[isError]    ${False}
    Should Contain    ${del_res}[content][0][text]    deleted successfully

MCP BREAD Lifecycle: Nodes and Edges
    [Documentation]    Exercises node and edge BREAD tools over MCP JSON-RPC.
    [Tags]    mcp    bread    nodes    edges

    # 1. node_add
    ${node_args}=    Create Dictionary    id=dudley_buck    label=Scientist    description=Dudley Buck was an MIT researcher in cryogenics and cryotrons.
    ${node_add_res}=    Mcp Call Tool    tool_name=node_add    arguments=${node_args}
    Should Be Equal    ${node_add_res}[isError]    ${False}

    # 2. node_read
    ${node_read_args}=    Create Dictionary    id=dudley_buck
    ${node_read_res}=    Mcp Call Tool    tool_name=node_read    arguments=${node_read_args}
    Should Be Equal    ${node_read_res}[isError]    ${False}
    Should Contain    ${node_read_res}[content][0][text]    Scientist

    # 3. edge_add
    ${edge_args}=    Create Dictionary    source_id=buzz_aldrin    predicate=STUDIED_AT    target_id=dudley_buck    auto_create_nodes=${True}
    ${edge_add_res}=    Mcp Call Tool    tool_name=edge_add    arguments=${edge_args}
    Should Be Equal    ${edge_add_res}[isError]    ${False}

    # 4. edge_read
    ${edge_read_args}=    Create Dictionary    source_id=buzz_aldrin    predicate=STUDIED_AT    target_id=dudley_buck
    ${edge_read_res}=    Mcp Call Tool    tool_name=edge_read    arguments=${edge_read_args}
    Should Be Equal    ${edge_read_res}[isError]    ${False}

    # 5. edge_delete
    ${edge_del_args}=    Create Dictionary    source_id=buzz_aldrin    predicate=STUDIED_AT    target_id=dudley_buck
    ${edge_del_res}=    Mcp Call Tool    tool_name=edge_delete    arguments=${edge_del_args}
    Should Be Equal    ${edge_del_res}[isError]    ${False}

    # 6. node_delete
    ${node_del_args}=    Create Dictionary    id=dudley_buck
    ${node_del_res}=    Mcp Call Tool    tool_name=node_delete    arguments=${node_del_args}
    Should Be Equal    ${node_del_res}[isError]    ${False}

MCP Pipeline Query Tool
    [Documentation]    Exercises graphrag_query tool to verify end-to-end RAG reasoning over MCP.
    [Tags]    mcp    pipeline    query

    ${query_args}=    Create Dictionary    question=Who flew the Apollo 11 command module Columbia in lunar orbit?
    ${query_res}=    Mcp Call Tool    tool_name=graphrag_query    arguments=${query_args}
    Should Be Equal    ${query_res}[isError]    ${False}
    Should Contain    ${query_res}[content][0][text]    Michael Collins
