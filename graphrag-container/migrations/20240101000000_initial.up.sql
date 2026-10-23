-- Initial migration to test sqlx migrate functionality
CREATE TABLE test_table (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL
);
