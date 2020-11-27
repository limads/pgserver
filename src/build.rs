use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Tokenizer, Token};
use sqlparser::dialect::keywords::Keyword;
use std::io;
use toml;
use std::fs::{self, File};
use std::io::{Read, Write};
use structopt::*;
use std::path::{PathBuf, Path};
use std::process::Command;
use std::ffi::OsStr;

#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    name : String,
    description : String,
    version : String
}

// Reads the current crate's Cargo.toml and extract the .control file required by Postgres.
// The "comment" field of the control file is read from the package.description Toml field.
// The "default_version" field of the control file is read from the package.version Toml field.
// The "relocatable" field is always set to true.
pub fn extract_crate_info(toml_path : &Path) -> Result<ExtensionInfo, String> {
    let mut info = String::new();
    let mut f = File::open(toml_path)
        .map_err(|e| format!("Could not read toml file: {}", e) )?;
    f.read_to_string(&mut info);
    let v : toml::Value = info.parse()
        .map_err(|e| format!("Could not parse toml file: {}", e) )?;
    let pkg = v.get("package").ok_or(format!(".toml file missing package entry"))?;
    match pkg {
        toml::Value::Table(tbl) => {
            match tbl.get("name") {
                Some(toml::Value::String(name)) => {
                    match tbl.get("description") {
                        Some(toml::Value::String(description)) => {
                            match tbl.get("version") {
                                Some(toml::Value::String(version)) => {
                                    Ok(ExtensionInfo{
                                        name : name.to_string(),
                                        description : description.to_string(),
                                        version : version.to_string()
                                    })
                                },
                                _ => Err(format!("Invalid version entry"))
                            }
                        },
                        _ => Err(format!("Invalid description entry"))
                    }
                },
                _ => Err(format!("Invalid package name entry"))
            }
        },
        _ => Err(format!("No package table entry found"))
    }
}

/// Copies SQL definition into target/release/postgres/${extname}-${extversion}.sql
pub fn write_extension_meta(target_dir : &Path, sql_path : &Path, ext_info : &ExtensionInfo) -> Result<(), String> {
    let sql_file_name = format!("{}--{}.sql", ext_info.name, ext_info.version);
    let mut sql_out_path = target_dir.to_path_buf();
    sql_out_path.push(sql_file_name);
    fs::copy(sql_path, sql_out_path).map_err(|e| format!("{}", e))?;

    /// Write control definitino into target/release/postgres/${extname}.control
    let mut control_path = target_dir.to_path_buf();
    control_path.push(format!("{}.control", ext_info.name));
    let mut info = String::new();
    info += &format!("# {} extension\n", ext_info.name);
    info += &format!("comment = '{}'\n", ext_info.description);
    info += &format!("default_version = '{}'\n", ext_info.version);
    info += &format!("module_pathname = '$libdir/{}'\n", ext_info.name);
    info += &format!("relocatable=true\n");
    let mut control_target = fs::OpenOptions::new()
        .truncate(true)
        .create(true)
        .write(true)
        .open(control_path)
        .map_err(|e| format!("Error opening control file target: {}", e))?;
    control_target.write_all(info.as_bytes())
        .map_err(|e| format!("Error writing to control file: {}", e))?;
    Ok(())
}

/// This is the public entry point that will be used to compile the extension.
pub fn compile_extension(
    target_dir : &Path,
    ext_info : &ExtensionInfo,
    extra_flags : Option<String>
) -> Result<(), String> {
    let mut src_path = target_dir.to_path_buf();
    src_path.push(format!("{}.c", ext_info.name));
    let mut sql_path = target_dir.to_path_buf();
    sql_path.push(format!("{}--{}.sql", ext_info.name, ext_info.version));
    let mut src_file = fs::OpenOptions::new()
        .truncate(true)
        .create(true)
        .write(true)
        .open(&src_path)
        .map_err(|e| format!("Unable to open src SQL file: {}", e))?;
    let c_wrapper = build_c_wrapper(&sql_path)?;
    src_file.write_all(c_wrapper.as_bytes()).map_err(|e| format!("{}", e))?;
    
    // Compile object file
    let obj_name = compile_object(&target_dir, &src_path, &ext_info)?;
    
    // Compile so file
    compile_so(&target_dir, &ext_info, &obj_name[..], extra_flags)?;
    Ok(())
}

pub fn deploy_extension(target_dir : &Path, ext_info : &ExtensionInfo) -> Result<(), String> {
    let pkg_lib_dir = pg_dir("--pkglibdir")?;
    let share_dir = format!("{}/extension", pg_dir("--sharedir")?); //pkglibdir sharedir/extension libdir ??

    let so_name = format!("lib{}.so", ext_info.name);
    let sql_name = format!("{}--{}.sql", ext_info.name, ext_info.version);
    let control_name = format!("{}.control", ext_info.name);
    let mut src_so = target_dir.to_path_buf();
    src_so.push(so_name.clone());
    let mut src_sql = target_dir.to_path_buf();
    src_sql.push(sql_name.clone());
    let mut src_control = target_dir.to_path_buf();
    src_control.push(control_name.clone());

    let mut dst_so = PathBuf::new();
    dst_so.push(pkg_lib_dir);
    dst_so.push(so_name);
    let mut dst_sql = PathBuf::new();
    dst_sql.push(&share_dir);
    dst_sql.push(sql_name);
    let mut dst_control = PathBuf::new();
    dst_control.push(share_dir);
    dst_control.push(control_name);

    fs::copy(&src_so, &dst_so).map_err(|e| format!("Unable to copy .so file: {}", e))?;
    println!("{:?} copied into {:?}", src_so, dst_so);
    fs::copy(&src_sql, &dst_sql).map_err(|e| format!("Unable to copy .sql file: {}", e))?;
    println!("{:?} copied into {:?}", src_sql, dst_sql);
    fs::copy(&src_control, &dst_control).map_err(|e| format!("Unable to copy .contorl file: {}", e))?;
    println!("{:?} copied into {:?}", src_control, dst_control);
    Ok(())
}

// This will eventually be a part of a build script for any Rust crates
// that are exported as Postgres extensions. The build script for any crates
// that have the pgserver crate will analyze a top-level sql file containing
// the SQL definitions and generate the corresponding C wrapper.
//
// The actual compilation of the C wrapper will just compile the wrapper
// C library, which should link againt a static compilation unit containing the whole crate.
// rustc postgres/parse_native.rs -o postgres/parse_native --extern sqlparser -L target/release/deps
// This compilation flag can be distributed as a standard Makefile that can be installed on the
// server via make/make install.
fn check_c_language(tk_iter : &mut std::vec::Drain<'_, Token>) -> bool {
    while let Some(fn_tk) = tk_iter.next() {
        match fn_tk {
            Token::Word(w) => if w.keyword == Keyword::LANGUAGE {
                match tk_iter.nth(1) {
                    Some(Token::Word(w)) => {
                        if &w.value[..] == "c" || &w.value[..] == "C" {
                            return true;
                        } else {
                            return false;
                        }
                    },
                    _ => { return false; }
                }
            },
            Token::EOF | Token::SemiColon => { return false; },
            _ => { }
        }
    }
    false
}

/// Create a C wrapper based on an extension definition SQL file. All functions that are language c at this file
/// will generate a corresponding macro function definition required by the Postgre server.
/// let sql = "create function add(a integer, b integer) returns integer as 'file.so', 'function' language c strict";
fn build_c_wrapper(sql_path : &Path) -> Result<String, String> {
    let mut sql = String::new();
    let mut f = File::open(&sql_path)
        .map_err(|e| format!("Could not read toml file: {}", e) )?;
    f.read_to_string(&mut sql);

    let dialect = PostgreSqlDialect {};
    let mut tokenizer = Tokenizer::new(&dialect, &sql);
    let mut tokens = tokenizer.tokenize().unwrap();
    let mut tk_iter = tokens.drain(..);
    let mut fn_names : Vec<String> = Vec::new();
    while let Some(tk) = tk_iter.next() {
        match tk {
            Token::Word(w) => if w.keyword == Keyword::CREATE {
                match tk_iter.nth(1) {
                    Some(Token::Word(w)) => if w.keyword == Keyword::FUNCTION {
                        match tk_iter.nth(1) {
                            Some(Token::Word(w)) => if check_c_language(&mut tk_iter) {
                                fn_names.push(w.value.clone());
                            },
                            _ => { }
                        }
                    },
                    _ => { }
                }
            },
            _ => { }
        }
    }

    let mut c_wrapper = String::new();
    c_wrapper += &format!("#include \"postgres.h\"\n#include \"fmgr.h\"\n\nPG_MODULE_MAGIC;\n\n");
    for f in fn_names {
        c_wrapper += &format!("PG_FUNCTION_INFO_V1({});\n\n", f);
    }
    Ok(c_wrapper)
}

fn pg_dir(flag : &str) -> Result<String, String> {
    let opt_dir = Command::new("pg_config")
        .arg(flag)
        .output()
        .map(|out| {
            if out.status.success() {
                Some(String::from_utf8(out.stdout).unwrap().trim().to_string())
            } else {
                None
            }
        }).map_err(|e| format!("Error running pg_config for flag {}: {}", flag, e) )?;
    let dir = opt_dir.ok_or(format!("Could not determine PostgreSQL {}", flag))?;
    println!("Found Postgres directory: {} = {}", flag, dir);
    Ok(dir)
}

/// Compiles wrapper source into object file, returning its name
fn compile_object(
    target_dir : &Path,
    src_path : &Path,
    ext_info : &ExtensionInfo
) -> Result<String, String> {
    let mut obj_out = target_dir.to_path_buf();
    obj_out.push(format!("{}.o", ext_info.name));
    let src_flag = format!("{}", src_path.display());
    let link_crate = format!("-l{}", ext_info.name);
    let link_search = "-Ltarget/release";
    let obj_out_flag = format!("{}", obj_out.display());
    let include_dir = pg_dir("--includedir-server")?;
    let pg_include = format!("-I{}", include_dir);
    let mut o_flags : Vec<&str> = Vec::new();
    o_flags.extend(["-c", &src_flag[..], "-fPIC", "-o", &obj_out_flag[..]].iter());
    o_flags.extend([&pg_include[..], &link_search[..], &link_crate[..]].iter());
    println!("gcc flags (compile .o): {:?}", o_flags);
    println!("gcc call: {:?}", Command::new("gcc").args(&o_flags[..]));
    let compile_obj_out = Command::new("gcc")
        .args(&o_flags[..])
        .output()
        .map_err(|e| format!("Error invoking gcc to compile .o file: {}", e))?;
    if compile_obj_out.status.success() {
        Ok(obj_out_flag)
    } else {
        Err(format!("gcc compilation error: {}", String::from_utf8(compile_obj_out.stderr).unwrap()))
    }
}

fn compile_so(
    target_dir : &Path,
    ext_info : &ExtensionInfo,
    obj_out_flag : &str,
    extra_flags : Option<String>
) -> Result<(), String> {
    let mut so_out = target_dir.to_path_buf();
    so_out.push(format!("lib{}.so", ext_info.name));
    let so_out_flag = format!("{}", so_out.display());
    let whole_a = "-Wl,--whole-archive";
    let static_target = format!("target/release/lib{}.a", ext_info.name);
    let no_whole_a = "-Wl,--no-whole-archive";
    let mut so_flags = Vec::new();
    so_flags.extend([&obj_out_flag[..], "-shared", "-o", &so_out_flag[..]].iter());
    so_flags.extend([whole_a, &static_target[..], no_whole_a].iter());
    println!("gcc flags (compile .so): {:?}", so_flags);
    println!("gcc call: {:?}", Command::new("gcc").args(&so_flags[..]));
    if let Some(extra) = extra_flags.as_ref() {
        for flag in extra.split(' ') {
            so_flags.push(&flag[..]);
        }
    }
    let compile_so_out = Command::new("gcc")
        .args(&so_flags[..])
        .output()
        .map_err(|e| format!("Error inkovking gcc to compile .so file: {}", e))?;
    if compile_so_out.status.success() {
        Ok(())
    } else {
        Err(format!("gcc compilation error: {}", String::from_utf8(compile_so_out.stderr).unwrap()))
    }
}

