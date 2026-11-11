-- The sqlite row's test workload (WP-1277): the shell over a scripted,
-- deterministic workload (no random(), no clock), checked line for line
-- against sqlite-workload.expected. It exercises the b-tree (bulk inserts,
-- deletes, a vacuum), indexes and the planner, joins, aggregates, window
-- functions, recursive CTEs, JSON, FTS5, R*Tree, triggers, views, upsert,
-- generated columns, math functions, printf and floating-point formatting,
-- date arithmetic over fixed dates, blobs and collations.
.bail on
.mode list
.headers off
PRAGMA page_size = 1024;
PRAGMA cache_size = 64;

-- b-tree: 50000 rows through a recursive CTE, an index, deletes, vacuum
CREATE TABLE t(id INTEGER PRIMARY KEY, a INTEGER, b TEXT, c REAL);
WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 50000)
INSERT INTO t SELECT i, (i * 7919) % 10007, printf('row-%06d', (i * 31) % 50000), i / 7.0 FROM n;
CREATE INDEX t_a ON t(a);
CREATE INDEX t_b ON t(b DESC);
SELECT count(*), sum(a), min(b), max(b), printf('%.6f', sum(c)) FROM t;
SELECT count(*) FROM t WHERE a BETWEEN 100 AND 200;
SELECT group_concat(id, ',') FROM (SELECT id FROM t WHERE a = 4242 ORDER BY id);
DELETE FROM t WHERE id % 3 = 0;
SELECT count(*), total(a) FROM t;
VACUUM;
PRAGMA integrity_check;
SELECT b FROM t WHERE b > 'row-049990' ORDER BY b LIMIT 3;

-- joins and aggregates
CREATE TABLE dept(id INTEGER PRIMARY KEY, name TEXT COLLATE NOCASE);
CREATE TABLE emp(id INTEGER PRIMARY KEY, dept INTEGER REFERENCES dept, salary INTEGER, name TEXT);
INSERT INTO dept VALUES (1, 'Eng'), (2, 'ops'), (3, 'Sales'), (4, 'empty');
WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 300)
INSERT INTO emp SELECT i, 1 + i % 3, 1000 + (i * 37) % 900, 'e' || i FROM n;
SELECT d.name, count(e.id), sum(e.salary), max(e.salary)
  FROM dept d LEFT JOIN emp e ON e.dept = d.id GROUP BY d.id ORDER BY d.name;
SELECT name FROM dept WHERE name = 'OPS';
SELECT d.name, e.name, e.salary FROM emp e JOIN dept d ON d.id = e.dept
  WHERE e.salary = (SELECT max(salary) FROM emp WHERE dept = e.dept) ORDER BY d.id, e.id;
SELECT dept, count(*) FROM emp GROUP BY dept HAVING avg(salary) > 1440 ORDER BY dept;

-- window functions
SELECT id, salary, rank() OVER w, sum(salary) OVER (w ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING),
       lag(salary, 1, -1) OVER w, ntile(4) OVER w
  FROM emp WHERE dept = 2 AND id < 40 WINDOW w AS (ORDER BY salary, id) ORDER BY id;
SELECT dept, group_concat(id, ' ') FROM (
  SELECT dept, id, row_number() OVER (PARTITION BY dept ORDER BY salary DESC, id) AS r FROM emp
) WHERE r <= 3 GROUP BY dept ORDER BY dept;

-- recursive CTEs: Fibonacci, a Mandelbrot, a tree walk
WITH RECURSIVE f(n, a, b) AS (SELECT 0, 0, 1 UNION ALL SELECT n + 1, b, a + b FROM f WHERE n < 80)
SELECT n, a FROM f WHERE n IN (10, 40, 80);
WITH RECURSIVE
  xaxis(x) AS (VALUES(-2.0) UNION ALL SELECT x + 0.1 FROM xaxis WHERE x < 1.2),
  yaxis(y) AS (VALUES(-1.0) UNION ALL SELECT y + 0.2 FROM yaxis WHERE y < 1.0),
  m(iter, cx, cy, x, y) AS (
    SELECT 0, x, y, 0.0, 0.0 FROM xaxis, yaxis
    UNION ALL
    SELECT iter + 1, cx, cy, x * x - y * y + cx, 2.0 * x * y + cy FROM m
     WHERE (x * x + y * y) < 4.0 AND iter < 28),
  m2(iter, cx, cy) AS (SELECT max(iter), cx, cy FROM m GROUP BY cx, cy),
  a(t) AS (SELECT group_concat(substr(' .+*#', 1 + min(iter / 7, 4), 1), '') FROM m2 GROUP BY cy)
SELECT group_concat(rtrim(t), char(10)) FROM a;
CREATE TABLE node(id INTEGER PRIMARY KEY, parent INTEGER, label TEXT);
WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 63)
INSERT INTO node SELECT i, i / 2, 'n' || i FROM n;
WITH RECURSIVE walk(id, depth, path) AS (
  SELECT id, 0, label FROM node WHERE parent = 0
  UNION ALL
  SELECT node.id, depth + 1, path || '/' || node.label FROM node JOIN walk ON node.parent = walk.id
  ORDER BY 2 DESC, 1)
SELECT depth, count(*), max(path) FROM walk GROUP BY depth ORDER BY depth;

-- JSON
SELECT json_object('a', 1, 'b', json_array(1, 2.5, 'x', NULL), 'c', json('{"d":true}'));
SELECT json_extract('{"a":{"b":[10,20,30]}}', '$.a.b[#-1]'), json_type('[1,{"x":2}]', '$[1].x');
SELECT key, value FROM json_each('{"k1":1,"k2":"two","k3":[3]}') ORDER BY key;
SELECT json_group_array(id) FROM (SELECT id FROM emp WHERE salary > 1890 ORDER BY id);
SELECT json_set('{"a":1}', '$.b', 2, '$.a', 'one'), json_remove('[0,1,2,3]', '$[1]', '$[1]');
SELECT json_valid('{"a":'), json_quote('he said "hi"'), '{"a":[1,2]}' -> '$.a', '{"a":[1,2]}' ->> '$.a[1]';

-- FTS5
CREATE VIRTUAL TABLE doc USING fts5(title, body);
INSERT INTO doc VALUES
  ('compilers', 'a compiler translates source code into machine code'),
  ('ovens', 'an oven bakes bread at a constant temperature'),
  ('constexpr', 'constant evaluation runs code at compile time in the compiler'),
  ('kilns', 'a kiln fires pottery at a very high temperature');
SELECT title FROM doc WHERE doc MATCH 'compil*' ORDER BY rank;
SELECT title, highlight(doc, 1, '[', ']') FROM doc WHERE doc MATCH 'temperature' ORDER BY title;
SELECT count(*) FROM doc WHERE doc MATCH 'code NOT machine';

-- R*Tree
CREATE VIRTUAL TABLE box USING rtree(id, minx, maxx, miny, maxy);
WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM n WHERE i < 399)
INSERT INTO box SELECT i, i % 20, i % 20 + 1.5, i / 20, i / 20 + 1.5 FROM n;
SELECT count(*), sum(id) FROM box WHERE minx <= 10 AND maxx >= 9 AND miny <= 5 AND maxy >= 4;

-- triggers, views, upsert, generated columns
CREATE TABLE stock(sku TEXT PRIMARY KEY, qty INTEGER NOT NULL, price REAL,
                   value REAL GENERATED ALWAYS AS (qty * price) STORED);
CREATE TABLE audit(seq INTEGER PRIMARY KEY, sku TEXT, old INTEGER, new INTEGER);
CREATE TRIGGER stock_audit AFTER UPDATE OF qty ON stock
  BEGIN INSERT INTO audit(sku, old, new) VALUES (new.sku, old.qty, new.qty); END;
CREATE VIEW low AS SELECT sku, qty FROM stock WHERE qty < 5;
INSERT INTO stock(sku, qty, price) VALUES ('bolt', 10, 0.25), ('nut', 3, 0.1), ('gear', 1, 12.5);
INSERT INTO stock(sku, qty, price) VALUES ('bolt', 5, 0.3)
  ON CONFLICT(sku) DO UPDATE SET qty = qty + excluded.qty, price = excluded.price;
UPDATE stock SET qty = qty - 1 WHERE sku IN (SELECT sku FROM low);
SELECT sku, qty, printf('%.2f', value) FROM stock ORDER BY sku;
SELECT * FROM audit ORDER BY seq;
SELECT group_concat(sku) FROM low;
BEGIN;
DELETE FROM stock;
ROLLBACK;
SELECT count(*) FROM stock;
SAVEPOINT s1;
UPDATE stock SET qty = 0;
ROLLBACK TO s1;
RELEASE s1;
SELECT sum(qty) FROM stock;

-- math, printf, floating point, integers at the edges
SELECT round(pi(), 10), sqrt(2.0), exp(1.0), ln(10.0), pow(2, 0.5), atan2(1, 1), floor(-2.5), ceil(-2.5);
SELECT 9223372036854775807 + 0, -9223372036854775808, 9223372036854775807 + 1, 1e308 * 10, -0.0, 1.0 / 3;
SELECT printf('%5d|%-5d|%05.1f|%e|%g|%x|%o|%c|%q|%Q|%w', 42, 42, 3.14159, 12345.678, 0.0001, 255, 8, 'z', 'it''s', NULL, 'a"b');
SELECT printf('%.20f', 0.1), printf('%!.20g', 0.1), 0.1 + 0.2, 100000000000000000000.0, 1.5e-10;
SELECT 7 / 2, 7 % 3, -7 / 2, -7 % 3, 7.0 / 2, 1 << 62, (1 << 63) >> 1, ~5, 6 & 3, 6 | 3;
SELECT cast('12abc' AS INTEGER), cast(3.99 AS INTEGER), cast(-3.99 AS INTEGER), cast('0x1F' AS INTEGER), 0x1F, typeof(1e3), typeof('1');

-- dates over fixed values
SELECT date('2026-11-10', '+1 month', 'start of month', '-1 day'), julianday('2000-01-01 12:00:00'),
       strftime('%Y-%j %w %H:%M:%S', '2024-02-29 23:59:59', '+1 second'), unixepoch('1970-01-02'),
       datetime(1700000000, 'unixepoch'), timediff('2026-03-01', '2025-03-01');

-- strings, blobs, collations
SELECT upper('kiln'), lower('ÄBC'), length('héllo'), octet_length('héllo'), hex('héllo'), unicode('é'), char(107, 105, 108, 110);
SELECT substr('constexpr', -4), replace('a.b.c', '.', '::'), instr('compile time', 'time'), trim('  x  '), ltrim('xxyx', 'x'), format('%s-%d', 'v', 26);
SELECT hex(zeroblob(4)), hex(x'00ff10' || x'20'), length(x'0102'), quote(x'abcd'), typeof(randomblob(0)), unhex('6b696c6e');
CREATE TABLE s(v TEXT);
INSERT INTO s VALUES ('b'), ('B'), ('a'), ('A'), ('é'), ('c ');
SELECT group_concat(v, '') FROM (SELECT v FROM s ORDER BY v);
SELECT group_concat(v, '') FROM (SELECT v FROM s ORDER BY v COLLATE NOCASE, v);
SELECT group_concat(v, '') FROM (SELECT v FROM s ORDER BY v COLLATE RTRIM DESC, v);
SELECT 'a' LIKE 'A', 'a' GLOB 'A', 'abc' LIKE 'a_c', 'a%c' LIKE 'a\%c' ESCAPE '\', 'x' IN ('x', 'y'), NULL IS NULL, NULL = NULL;

-- the planner picks the index
EXPLAIN QUERY PLAN SELECT * FROM t WHERE a = 5;
SELECT 'done';
