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
use pgdatum::build::{self, ExtensionInfo};

#[derive(StructOpt, Debug)]
pub struct PgInstall {

    #[structopt(name="PATH")]
    path : Option<PathBuf>,

    #[structopt(long)]
    extra : Option<String>
}

fn main() -> Result<(), String> {
    let pg_install = PgInstall::from_args();
    let crate_path = pg_install.path.clone()
        .unwrap_or_else(|| { let mut buf = PathBuf::new(); buf.push("."); buf });
    let mut entries : Vec<_> = crate_path.read_dir()
        .map_err(|e| format!("Unable to read crate directory entries: {}", e) )?
        .filter_map(|e| e.ok() )
        .collect();
    println!("{:?}", entries);
    if let Some(sql) = entries.iter().find(|e| e.path().file_name().and_then(|f| f.to_str()) == Some("sql") ) {
        let sql_entries : Vec<_> = sql.path().read_dir()
            .map_err(|e| format!("Unable to view content of sql directory: {}", e))?
            .filter_map(|e| e.ok() )
            .filter(|p| p.path().extension() == Some(&OsStr::new("sql")))
            .collect();
        match sql_entries.len() {
            0 => Err(format!("Missing SQL script file at [crate]/sql folder"))?,
            1 => {
                let mut toml_path = crate_path.clone();
                toml_path.push("Cargo.toml");
                let ext_info = build::extract_crate_info(&toml_path)?;
                let mut target_dir = crate_path.clone();
                target_dir.push("target");
                target_dir.push("release");
                target_dir.push("postgres");
                if !target_dir.exists() {
                    fs::create_dir(&target_dir)
                        .map_err(|e| format!("Unable to create target extenion directory: {}", e))?;
                }
                build::write_extension_meta(&target_dir, &sql_entries[0].path(), &ext_info)?;
                build::compile_extension(&target_dir, &ext_info, pg_install.extra.clone())?;
                build::deploy_extension(&target_dir, &ext_info)?;
                println!("Execute \"CREATE EXTENSION {};\" in your database to access the extension.",
                    ext_info.name
                );
                Ok(())
            },
            _ => Err(format!("Multiple SQL script files at [crate]/sql folder"))
        }
    } else {
        Err(format!("Missing sql directory at crate root"))
    }
}


