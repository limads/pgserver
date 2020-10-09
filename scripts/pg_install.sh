#!/usr/bin/bash

gcc_extra=$1

# Allow the user to pass the crate directory as first argument.
# crate_dir=.

if [ ! -e .pg_install ]; then
    mkdir .pg_install
fi

compile_parse_native=1
# compile_crate_info=1

if [ ! -e .pg_install/parse_native ]; then
    compile_parse_native=$(rustc src/bin/parse_native.rs -o .pg_install/parse_native \
        --extern sqlparser -L target/release/deps)
fi

# if [ ! -e .pg_install/crate_info ]; then
#	compile_crate_info=$(rustc src/bin/crate_info.rs -o .pg_install/crate_info --extern toml -L target/release/deps)
# fi

if [ compile_parse_native ]; then #|| [ compile_crate_info ]; then
    echo "Could not compile script. Make sure your crate depends on pgdatum and is compiled as release mode."
    exit 1;
fi

extname=$(basename "$PWD")

cat sql/$(ls sql) | ./.pg_install/parse_native > .pg_install/${extname}.c

# Compile 'helper' module - Carries a few Postgre types and functions, mostly for
# variable-length SQL types (text, bytea, etc). This static library is used by the pgserver.rs
# module to allocate data using palloc and convert between Postgre's types and slices.
# gcc -c src/api/pg_helper.c -o target/c/pghelper.o -I$(pg_config --includedir-server)
# ar rcs target/c/libpghelper.a target/c/pghelper.o

# Verify symbols w/ nm -a target/release/libbayes.so
# Should be build with cargo build --release --features "api pgext"
gcc -c src/api/mod.c -fPIC -o .pg_install/${extname}.o \
	-I$(pg_config --includedir-server) -Ltarget/release -l${extname}
gcc .pg_install/bayes.o -shared -o target/c/libbayes.so \
    -Wl,--whole-archive target/release/libbayes.a -Wl,--no-whole-archive \
    ${gcc_extra}

rm target/c/bayes.o

# Deploy
sudo cp target/c/libbayes.so $(pg_config --pkglibdir)
sudo cp target/c/libbayes.md $(echo $(pg_config --docdir)/extension)

ext_path=${pg_config --sharedir}
pkg_path=${pg_config --pkglibdir}
tar -xzf extension_out [extension]
cp extension.control ext_path
cp extension-0.0.1.sql ext_path
cp extension.so pkg_path


