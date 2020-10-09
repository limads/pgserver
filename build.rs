use std::env;
use std::fs;
use cc;

fn main() {
    cc::Build::new()
        .file("src/pg_helper.c")
        .include("/usr/include/postgresql/11/server")
        .compile("pghelper");
}
