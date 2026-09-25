# OrchidDB CLI

A Rust CLI with bundled DuckDB 1.5.2 and the official DuckDB Iceberg extension
loaded by default. Graph queries compile to SQL and results use native Arrow
batches. First use installs the signed Iceberg extension and requires network
access to extensions.duckdb.org; subsequent runs use the local extension cache.
It fails clearly if installation/loading fails. The extension is downloaded,
not statically embedded in the executable. [DuckDB Iceberg documentation](https://duckdb.org/docs/stable/core_extensions/iceberg/overview).

```sh
cargo run -- query examples/people.json --init examples/setup.sql --format table
cargo run -- query examples/people.json --init examples/setup.sql > results.arrow
cargo run -- compile examples/people.json
```

`query` defaults to Arrow IPC on stdout; diagnostics use stderr. `--format table`
prints batches for humans. `--database FILE` opens a persistent DuckDB database.
`--init setup.sql` executes your SQL first, allowing credentials, plugins, UDFs,
views over `iceberg_scan`, and attached catalogs. Treat setup SQL as executable
application code. Schema/mappings in the request must match those sources.
`--no-iceberg` explicitly disables extension setup for queries that do not use it.
Compile mode never opens a database or installs extensions.

For Iceberg, create a view in setup SQL such as:

```sql
CREATE VIEW people AS
SELECT id, name FROM iceberg_scan('/path/to/table/metadata/v1.metadata.json');
```

Then use the same mapping in examples/people.json. No data is copied into a graph
store. The CLI owns its connection; embedding applications should use
[OrchidDB-rust](https://github.com/OrchidDB/OrchidDB-rust) for caller-owned sessions.
Arrow batches avoid per-cell object conversion, but DuckDB query execution may
materialize results internally. Supported graph language scope is the compiler's
SQL-lowerable subset, not every managed-runtime feature.

## Build, test, release

`cargo test` includes an actual default Iceberg load (network access on first run).
`cargo build --release` bundles DuckDB. A matching external DuckDB library can be
used for development with `DUCKDB_LIB_DIR` and `--no-default-features`.
The tag-triggered workflow builds Linux/macOS binaries, tests each platform,
validates the version, and creates a draft GitHub release with checksums.
No registry credentials are needed beyond the repository Actions token.
Crates.io publication is disabled until core and its modified vendored parser
can be published as versioned dependencies. Source Git dependencies are pinned.
The existing OrchidDB license applies; see LICENSE.md.
