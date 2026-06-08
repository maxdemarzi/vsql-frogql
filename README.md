# vsql_frogql - froGQL Extension for VillageSQL

A pure Rust extension for VillageSQL (a MySQL fork) that integrates the **froGQL** graph database engine. This extension allows you to perform graph traversals on `.gdb` graph databases directly from SQL queries via the `frogql_traverse` function, returning a JSON array of results that can be joined with standard relational tables using MySQL's `JSON_TABLE()`.

## Features

- **Direct Graph Traversal**: Query graph databases using froGQL's Cypher-like syntax within standard SQL statements.
- **Relational Joins**: Mix standard relational data and graph data seamlessly in a single `JOIN` via `JSON_TABLE()`.
- **Pure Rust Implementation**: Built entirely in Rust using the `vsql-rust-sdk`.
- **Large Output Support**: Pre-configured with a large 16MB result buffer size to handle big JSON result sets without truncation.

---

## Prerequisites

- **Rust toolchain** (latest stable release recommended)
- **VillageSQL Server** (source or build tree containing `mysql-test` directory and compiled binaries)

---

## Compilation and Packaging

The compilation script compiles the Rust library in release mode and packages it as a `.veb` archive.

To build the extension:

1. Define the `VillageSQL_BUILD_DIR` environment variable pointing to your VillageSQL build tree.
2. Run `build.sh`:

```bash
export VillageSQL_BUILD_DIR=/home/maxdemarzi/build/villagesql
./build.sh
```

This will:
1. Compile the crate using `cargo build --release`.
2. Package the compiled shared library and the `manifest.json` into `build/vsql_frogql.veb`.
3. Copy/install the `.veb` package to the VillageSQL build tree under `${VillageSQL_BUILD_DIR}/veb_output_directory`.

---

## Installation

Once packaged and installed into the server's VEB directory, connect to your VillageSQL server and run:

```sql
INSTALL EXTENSION vsql_frogql;
```

To uninstall the extension:

```sql
UNINSTALL EXTENSION vsql_frogql;
```

---

## Usage Examples

First, register a database name pointing to a specific file:

```sql
-- Register a database name pointing to a specific file
SELECT frogql_register_database('disney', '/var/lib/mysql-files/disney.gdb');
```

**Output:**
```
+-----------------------------------------------------------------------+
| frogql_register_database('disney', '/var/lib/mysql-files/disney.gdb') |
+-----------------------------------------------------------------------+
| OK                                                                    |
+-----------------------------------------------------------------------+
```

### Standalone Traversal Example

You can execute a graph query with relationship traversal directly to inspect the returned JSON payload:

```sql
SELECT frogql_traverse('disney', 'MATCH (t1:characters)<-[name:NAME]-(t2:director) RETURN t1.song, t2.director LIMIT 3');
```

**Output:**
```
+------------------------------------------------------------------------------------------------------------------+
| frogql_traverse('disney', 'MATCH (t1:characters)<-[name:NAME]-(t2:director) RETURN t1.song, t2.director LIMIT 3')|
+------------------------------------------------------------------------------------------------------------------+
| [["Some Day My Prince Will Come","David Hand"],["When You Wish upon a Star","Ben Sharpsteen"],[null,"full credits"]]|
+------------------------------------------------------------------------------------------------------------------+
```

### Extracting a Plain String Value

To clean up the output and retrieve the raw string value (removing the JSON arrays), you can convert the output to `utf8mb4` and use MySQL's `JSON_VALUE()` function:

```sql
SELECT JSON_VALUE(
  CONVERT(frogql_traverse('disney', 'MATCH (t1:characters)<-[name:NAME]-(t2:director) RETURN t1.song, t2.director LIMIT 3') USING utf8mb4),
  '$[0][0]'
) AS song_name;
```

**Output:**
```
+------------------------------+
| song_name                    |
+------------------------------+
| Some Day My Prince Will Come |
+------------------------------+
```

### Relational JOIN Example

To combine graph database queries with standard relational tables, use `JSON_TABLE()` to unpack the traversal results:

```sql
-- Query and JOIN standard SQL data with the graph traversal
SELECT directors.born, graph.song
FROM (
  SELECT 'David Hand' AS director, 1900 AS born
  UNION ALL
  SELECT 'Ben Sharpsteen' AS director, 1895 AS born
) AS directors
JOIN JSON_TABLE(
  CONVERT(frogql_traverse('disney', 'MATCH (t1:characters)<-[name:NAME]-(t2:director) RETURN t1.song, t2.director') USING utf8mb4),
  '$[*]' COLUMNS(
    song VARCHAR(255) PATH '$[0]',
    director VARCHAR(255) PATH '$[1]'
  )
) AS graph ON directors.director = graph.director
ORDER BY directors.born, graph.song;
```

**Output:**
```
+------+------------------------------+
| born | song                         |
+------+------------------------------+
| 1895 | Baby Mine                    |
| 1895 | When You Wish upon a Star    |
| 1900 | Love Is a Song               |
| 1900 | Some Day My Prince Will Come |
+------+------------------------------+
```

### Path Resolution
When querying a graph database via `frogql_traverse(db, query)`:
1. If the database name `db` is mapped to a file path via `frogql_register_database(db, path)`, that registered path is used.
2. Otherwise, `db` is treated as a direct file path (either absolute or relative to the server data directory).
3. Path shortcuts starting with `~` (e.g. `~/vsql-frogql/examples/my_graph.gdb`) are automatically expanded to the user's home directory.

For example, you can register and query databases using tilde paths directly:
```sql
SELECT frogql_register_database('disney', '~/vsql-frogql/examples/disney.gdb');
```

**Output:**
```
+------------------------------------------------------------------------+
| frogql_register_database('disney', '~/vsql-frogql/examples/disney.gdb') |
+------------------------------------------------------------------------+
| OK                                                                     |
+------------------------------------------------------------------------+
```

```sql
SELECT frogql_traverse('disney', 'MATCH (t1:characters) RETURN t1.song LIMIT 3');
```

**Output:**
```
+-----------------------------------------------------------------------------+
| frogql_traverse('disney', 'MATCH (t1:characters) RETURN t1.song LIMIT 3')   |
+-----------------------------------------------------------------------------+
| [["Some Day My Prince Will Come"],["When You Wish upon a Star"],[null]]     |
+-----------------------------------------------------------------------------+
```

---

## Running Tests

An integration test suite is included under `mysql-test`. You can execute it using the MySQL Test Runner (MTR) from your VillageSQL build folder:

```bash
cd /home/maxdemarzi/build/villagesql
./mysql-test/mysql-test-run.pl --suite=/home/maxdemarzi/vsql-frogql/mysql-test
```
