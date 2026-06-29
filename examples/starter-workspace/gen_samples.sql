-- Duckle sample-data generator.
--
-- The desktop app runs this once, through the provisioned DuckDB, when it seeds
-- a brand-new / empty workspace. The literal ${workspace} is substituted to the
-- workspace folder before the script runs. Every input is intentionally tiny (a
-- few hundred rows) so the bundled sample pipelines run instantly and the files
-- take well under a megabyte on disk.

-- orders.csv : small orders table for the CSV-filter and join+group-by starters.
COPY (
  SELECT
    1000 + i                                          AS order_id,
    'cust_' || (1 + (i % 8))                          AS customer,
    (['paid', 'pending', 'refunded'])[1 + (i % 3)]    AS status,
    round(20.0 + (i % 480) + (i % 100) * 0.01, 2)     AS amount,
    (['North', 'South', 'East', 'West'])[1 + (i % 4)] AS region,
    (DATE '2024-01-01' + (i % 28)::INTEGER)           AS created_at
  FROM range(40) t(i)
) TO '${workspace}/data/orders.csv' (HEADER, DELIMITER ',');

-- regions.csv : region -> manager lookup for the join sample.
COPY (
  SELECT * FROM (VALUES
    ('North', 'Alice Chen'),
    ('South', 'Bob Diaz'),
    ('East',  'Carol Singh'),
    ('West',  'Dan Park')
  ) v(region, manager)
) TO '${workspace}/data/regions.csv' (HEADER, DELIMITER ',');

-- customers.csv : customer dimension. Used as a join lookup in the enrichment
-- sample and split by segment in the for-each sample.
COPY (
  SELECT
    1 + i                                              AS customer_id,
    'Customer ' || (1 + i)                             AS customer_name,
    (['SMB', 'Enterprise', 'Consumer'])[1 + (i % 3)]   AS segment,
    1 + (i % 4)                                        AS cust_region_id
  FROM range(20) t(i)
) TO '${workspace}/data/customers.csv' (HEADER, DELIMITER ',');

-- orders.parquet : a larger order fact for the incremental and enrichment
-- samples (still only 500 rows).
COPY (
  SELECT
    1 + i                                              AS order_id,
    1 + (i % 20)                                       AS customer_id,
    1 + (i % 10)                                       AS product_id,
    1 + (i % 4)                                        AS region_id,
    round(10.0 + (i % 990) + (i % 100) * 0.01, 2)      AS amount,
    (['paid', 'pending', 'refunded'])[1 + (i % 3)]     AS status,
    (TIMESTAMP '2024-01-01 00:00:00' + to_hours(i::INTEGER)) AS created_at
  FROM range(500) t(i)
) TO '${workspace}/data/orders.parquet' (FORMAT parquet);

-- products.duckdb : product dimension stored in a DuckDB file (a join lookup).
ATTACH '${workspace}/data/products.duckdb' AS pdb;
CREATE OR REPLACE TABLE pdb.products AS
  SELECT
    1 + i                                              AS product_id,
    'Product ' || (1 + i)                              AS product_name,
    (['Widgets', 'Gadgets', 'Gizmos'])[1 + (i % 3)]    AS category,
    round(5.0 + i * 2.5, 2)                            AS price
  FROM range(10) t(i);
DETACH pdb;

-- regions.sqlite : region dimension stored in a SQLite file (a join lookup).
INSTALL sqlite; LOAD sqlite;
ATTACH '${workspace}/data/regions.sqlite' AS sdb (TYPE sqlite);
CREATE OR REPLACE TABLE sdb.regions AS
  SELECT * FROM (VALUES
    (1, 'North', 'Alice Chen'),
    (2, 'South', 'Bob Diaz'),
    (3, 'East',  'Carol Singh'),
    (4, 'West',  'Dan Park')
  ) v(region_id, region_name, manager);
DETACH sdb;

-- cdc.ducklake : a DuckLake-managed table with an insert history, so the
-- DuckLake CDC sample has a real change feed to read.
-- Attach with DuckLake's default data path (a <catalog>.files dir next to the
-- catalog) so the CDC reader, which attaches the same way, finds the data.
INSTALL ducklake; LOAD ducklake;
ATTACH 'ducklake:${workspace}/data/cdc.ducklake' AS lake;
CREATE TABLE lake.customers_cdc (
  customer_id  INTEGER,
  customer_name VARCHAR,
  segment      VARCHAR
);
INSERT INTO lake.customers_cdc
  SELECT
    1 + i,
    'Customer ' || (1 + i),
    (['SMB', 'Enterprise', 'Consumer'])[1 + (i % 3)]
  FROM range(10) t(i);
DETACH lake;
