installdir=$(HOME)/.weechat
testdir=./test_dir

.PHONY: all install install_test test run format clippy
all: src/*
	cargo build --release

all_debug: src/*
	cargo build

install: all | $(installdir)/plugins
	cp target/release/libweechat_discord.* $(installdir)/plugins

install_test: all_debug | $(testdir)/plugins
	cp target/debug/libweechat_discord.* $(testdir)/plugins

run: install
	weechat -a

test: install_test
	weechat -d $(testdir)

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
