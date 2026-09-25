use arrow::{ipc::writer::StreamWriter, util::pretty::pretty_format_batches};
use duckdb::Connection;
use orchiddb_client::compile_json;
use std::{
    error::Error,
    fs,
    io::{self, Write},
};

const HELP: &str = "OrchidDB: graph queries over DuckDB + Iceberg
Usage: orchiddb compile REQUEST.json
       orchiddb query REQUEST.json [--database FILE] [--init SQL_FILE] [--format arrow|table] [--no-iceberg]
       orchiddb --version

REQUEST.json is compiler protocol v1, including query, mapping, and schema.
Iceberg loads by default for query; first use downloads the signed official extension.
--init executes application setup SQL (views, credentials, plugins, UDFs).
Arrow IPC is the default stdout format; diagnostics go to stderr.
";
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("orchiddb: {e}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        print!("{HELP}");
        return Ok(());
    };
    if command == "--help" || command == "-h" {
        print!("{HELP}");
        return Ok(());
    }
    if command == "--version" {
        println!("orchiddb {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if command != "query" && command != "compile" {
        return Err(format!("unknown command: {command}").into());
    }
    let request = args.next().ok_or("missing REQUEST.json")?;
    let mut database = None;
    let mut init = None;
    let mut format = "arrow".to_string();
    let mut iceberg = true;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--database" => database = Some(args.next().ok_or("missing database path")?),
            "--init" => init = Some(args.next().ok_or("missing SQL file")?),
            "--format" => format = args.next().ok_or("missing output format")?,
            "--no-iceberg" => iceberg = false,
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    if format != "arrow" && format != "table" {
        return Err("format must be arrow or table".into());
    }
    let compiled_json = compile_json(&fs::read_to_string(request)?)
        .await
        .map_err(io::Error::other)?;
    let compiled: serde_json::Value = serde_json::from_str(&compiled_json)?;
    let sql = compiled["sql"]
        .as_str()
        .ok_or("compiler did not return SQL")?;
    if command == "compile" {
        println!("{sql}");
        return Ok(());
    }
    if compiled["dialect"] != "duckdb" {
        return Err("CLI executes only DuckDB SQL".into());
    }
    let db = match database {
        Some(path) => Connection::open(path)?,
        None => Connection::open_in_memory()?,
    };
    if iceberg {
        if db.execute_batch("LOAD iceberg").is_err() {
            db.execute_batch("INSTALL iceberg; LOAD iceberg").map_err(|e| io::Error::other(format!("Iceberg could not be installed/loaded: {e}. First use requires network access to extensions.duckdb.org; use --no-iceberg only for queries that do not need Iceberg.")))?;
        }
        let loaded: bool = db.query_row(
            "SELECT loaded FROM duckdb_extensions() WHERE extension_name='iceberg'",
            [],
            |row| row.get(0),
        )?;
        if !loaded {
            return Err("Iceberg extension did not load".into());
        }
    }
    if let Some(path) = init {
        db.execute_batch(&fs::read_to_string(path)?)?;
    }
    let mut statement = db.prepare(sql)?;
    let batches = statement.query_arrow([])?;
    if format == "arrow" {
        let stdout = io::stdout();
        let mut writer = StreamWriter::try_new(stdout.lock(), &batches.get_schema())?;
        for batch in batches {
            writer.write(&batch)?;
        }
        writer.finish()?;
    } else {
        // Human output is deliberately one batch at a time, avoiding full-result collection.
        let mut out = io::stdout().lock();
        for batch in batches {
            writeln!(out, "{}", pretty_format_batches(&[batch])?)?;
        }
    }
    Ok(())
}
