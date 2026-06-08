// Copyright (c) 2026 VillageSQL Contributors
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use villagesql::{InValue, VdfReturn};
use gqlrust::runtime::engine::Runtime;
use gqlrust::store::lazy::LazyGraphStore;
use gqlrust::runtime::result::QueryResult;
use gqlrust::model::value::Value;
use std::path::Path;
use std::sync::RwLock;
use std::collections::HashMap;
use std::cell::RefCell;
use std::os::raw::c_char;

// Global map of database alias names to absolute/relative file paths
static DATABASE_MAP: RwLock<Option<HashMap<String, String>>> = RwLock::new(None);

thread_local! {
    static THREAD_STORE_CACHE: RefCell<HashMap<String, (LazyGraphStore, String)>> = RefCell::new(HashMap::new());
}

fn get_db_path(db_name: &str) -> String {
    let mut path_str = if let Ok(guard) = DATABASE_MAP.read() {
        if let Some(map) = guard.as_ref() {
            if let Some(path) = map.get(db_name) {
                path.clone()
            } else {
                db_name.to_string()
            }
        } else {
            db_name.to_string()
        }
    } else {
        db_name.to_string()
    };

    if path_str.starts_with("~/") || path_str == "~" {
        if let Ok(home) = std::env::var("HOME") {
            if path_str == "~" {
                path_str = home;
            } else {
                path_str = format!("{}/{}", home, &path_str[2..]);
            }
        }
    }
    path_str
}

fn register_db_path(db_name: String, file_path: String) {
    if let Ok(mut guard) = DATABASE_MAP.write() {
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(db_name, file_path);
    }
}

fn with_store<F, R>(db_name: &str, db_path_str: &str, f: F) -> Result<R, String>
where
    F: FnOnce(&LazyGraphStore) -> Result<R, String>,
{
    THREAD_STORE_CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        
        let need_open = if let Some((_, cached_path)) = map.get(db_name) {
            cached_path != db_path_str
        } else {
            true
        };
        
        if need_open {
            let db_path = Path::new(db_path_str);
            if !db_path.exists() {
                return Err(format!(
                    "frogql_traverse: database file does not exist at '{}'",
                    db_path.display()
                ));
            }
            let store = LazyGraphStore::open(db_path)
                .map_err(|e| format!("Failed to open graph database at '{}': {}", db_path.display(), e))?;
            map.insert(db_name.to_string(), (store, db_path_str.to_string()));
        }
        
        let (store, _) = map.get(db_name).unwrap();
        f(store)
    })
}

// Converts a Value into a serde_json::Value
fn value_to_json(val: &Value) -> serde_json::Value {
    match val {
        Value::Null => serde_json::Value::Null,
        Value::Int(n) => serde_json::Value::Number((*n).into()),
        Value::Float(f) => {
            if let Some(num) = serde_json::Number::from_f64(*f) {
                serde_json::Value::Number(num)
            } else {
                serde_json::Value::Null
            }
        }
        Value::Str(s) => serde_json::Value::String(s.clone()),
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::List(list) => {
            serde_json::Value::Array(list.iter().map(value_to_json).collect())
        }
        Value::Record(record) => {
            let mut map = serde_json::Map::new();
            for (k, v) in record {
                map.insert(k.clone(), value_to_json(v));
            }
            serde_json::Value::Object(map)
        }
        Value::Node(id) => serde_json::Value::Number((*id).into()),
        Value::Edge(id) => serde_json::Value::Number((*id).into()),
        Value::Path(path) => {
            serde_json::Value::Array(path.iter().map(value_to_json).collect())
        }
    }
}

fn frogql_traverse_impl(args: &[InValue]) -> VdfReturn {
    let db_name = match args.get(0) {
        Some(InValue::String(s)) => s,
        Some(InValue::Null) | None => return VdfReturn::null(),
        _ => return VdfReturn::error("frogql_traverse: db_name must be a STRING"),
    };

    let query_str = match args.get(1) {
        Some(InValue::String(s)) => s,
        Some(InValue::Null) | None => return VdfReturn::null(),
        _ => return VdfReturn::error("frogql_traverse: query must be a STRING"),
    };

    // Look up database name routing
    let db_path_str = get_db_path(db_name);

    let catch_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_store(db_name, &db_path_str, |store| {
            let compiled = gqlrust::compile_query(query_str)
                .map_err(|e| format!("Compilation error: {}", e))?;
            
            let rt = Runtime::new(store);
            let run_res = rt.run_query(&compiled, 0); // 0 = unlimited
            
            match run_res {
                QueryResult::Projected(rows) => {
                    let json_rows: Vec<serde_json::Value> = rows
                        .iter()
                        .map(|row| {
                            serde_json::Value::Array(row.iter().map(value_to_json).collect())
                        })
                        .collect();
                    serde_json::to_string(&json_rows)
                        .map_err(|e| format!("Serialization error: {}", e))
                }
                QueryResult::Raw(ir) => {
                    let mut json_rows = Vec::new();
                    for row in ir.rows {
                        let mut row_map = serde_json::Map::new();
                        for (k, v) in row.assignment.m {
                            let val = match v {
                                gqlrust::model::value::PathValue::Node(id) => serde_json::Value::Number(id.into()),
                                gqlrust::model::value::PathValue::EdgeDirectional(id) => serde_json::Value::Number(id.into()),
                                gqlrust::model::value::PathValue::EdgeUndirectional(id) => serde_json::Value::Number(id.into()),
                                gqlrust::model::value::PathValue::Nothing => serde_json::Value::Null,
                                _ => serde_json::Value::String(format!("{}", v)),
                            };
                            row_map.insert(k, val);
                        }
                        json_rows.push(serde_json::Value::Object(row_map));
                    }
                    serde_json::to_string(&json_rows)
                        .map_err(|e| format!("Serialization error: {}", e))
                }
            }
        })
    }));

    match catch_res {
        Ok(Ok(json_str)) => VdfReturn::string(json_str),
        Ok(Err(err_msg)) => VdfReturn::error(err_msg),
        Err(_) => VdfReturn::error("frogql_traverse: engine panicked during execution"),
    }
}

fn frogql_register_database_impl(args: &[InValue]) -> VdfReturn {
    let db_name = match args.get(0) {
        Some(InValue::String(s)) => s,
        Some(InValue::Null) | None => return VdfReturn::null(),
        _ => return VdfReturn::error("frogql_register_database: db_name must be a STRING"),
    };

    let file_path = match args.get(1) {
        Some(InValue::String(s)) => s,
        Some(InValue::Null) | None => return VdfReturn::null(),
        _ => return VdfReturn::error("frogql_register_database: file_path must be a STRING"),
    };

    register_db_path(db_name.to_string(), file_path.to_string());
    VdfReturn::string("OK")
}

#[no_mangle]
pub unsafe extern "C" fn vef_register(
    _arg: *const villagesql::sys::vef_register_arg_t,
) -> *mut villagesql::sys::vef_registration_t {
    let funcs: &[villagesql::FuncDescriptor] = &[
        villagesql::func!(
            frogql_traverse_impl,
            "frogql_traverse",
            [villagesql::Type::String, villagesql::Type::String] -> villagesql::Type::String
        ),
        villagesql::func!(
            frogql_register_database_impl,
            "frogql_register_database",
            [villagesql::Type::String, villagesql::Type::String] -> villagesql::Type::String
        ),
    ];
    let types: Vec<villagesql::TypeWithFuncs> = vec![];
    let reg = villagesql::build_registration(funcs, &types);
    
    // Explicitly assign extension name to avoid mismatch error on 0.0.3-dev server
    (*reg).deprecated_extension_name = concat!("vsql_frogql", "\0").as_ptr() as *const c_char;
    (*reg).deprecated_extension_version = concat!("0.1.0", "\0").as_ptr() as *const c_char;
    
    // Request a large buffer size (16MB) for the traverse JSON output
    let funcs_slice = std::slice::from_raw_parts_mut((*reg).funcs, (*reg).func_count as usize);
    (*funcs_slice[0]).buffer_size = 16 * 1024 * 1024;
    
    reg
}

#[no_mangle]
pub unsafe extern "C" fn vef_unregister(
    _arg: *const villagesql::sys::vef_unregister_arg_t,
    registration: *mut villagesql::sys::vef_registration_t,
) {
    villagesql::free_registration(registration);
}
