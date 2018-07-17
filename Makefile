installdir=$(HOME)/.weechat
testdir=./test_dir

.PHONY: all install install_test test run format clippy
all: src/*
	cargo build --release

all_debug: src/*
	cargo build

install: all | $(installdir)/plugins
	cp target/release/libweecord.* $(installdir)/plugins

install_test: all_debug | $(testdir)/plugins
	cp target/debug/libweecord.* $(testdir)/plugins

run: install
	weechat -a

test: install_test
	weechat -a -d $(testdir)

$(installdir):
	mkdir $@

$(installdir)/plugins: | $(installdir)
	mkdir $@

$(testdir):
	mkdir $@

$(testdir)/plugins: | $(testdir)
	mkdir $@

format:
	cargo fmt
	clang-format -style="{BasedOnStyle: google, IndentWidth: 4}" -i src/*.c