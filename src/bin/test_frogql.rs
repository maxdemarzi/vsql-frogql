use gqlrust::runtime::engine::Runtime;
use gqlrust::store::lazy::LazyGraphStore;
use gqlrust::runtime::result::QueryResult;
use std::path::Path;

fn main() {
    println!("Testing frogql query...");
    let db_path = Path::new("/home/maxdemarzi/vsql-frogql/frogql-src/examples/social_network_twitter.gdb");
    println!("Opening database at {:?}", db_path);
    let store = LazyGraphStore::open(db_path).unwrap();
    println!("Database opened. Compiling query...");
    let query_str = "MATCH (u:USER) RETURN u.username LIMIT 1";
    let compiled = gqlrust::compile_query(query_str).unwrap();
    println!("Query compiled. Running query...");
    let rt = Runtime::new(&store);
    let run_res = rt.run_query(&compiled, 0);
    println!("Query run finished.");
    match run_res {
        QueryResult::Projected(rows) => {
            println!("Projected rows: {}", rows.len());
        }
        QueryResult::Raw(ir) => {
            println!("Raw rows: {}", ir.rows.len());
        }
    }
}
