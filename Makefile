.PHONY: all build run deb install-deb uninstall clean help

NAME := whatsapp-desktop
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
DEB := $(wildcard target/debian/$(NAME)_*.deb)

all: deb

help:
	@echo "make build        release binary (target/release/$(NAME))"
	@echo "make run          run in debug mode"
	@echo "make deb          build .deb (target/debian/)"
	@echo "make install-deb  build and install the .deb with apt"
	@echo "make uninstall    remove the installed package"
	@echo "make clean        cargo clean"

build:
	cargo build --release

run:
	cargo run

# cargo-deb builds the release binary itself.
deb:
	@command -v cargo-deb >/dev/null || cargo install cargo-deb --locked
	cargo deb
	@ls -1 target/debian/$(NAME)_*.deb

install-deb: deb
	sudo apt install -y $(lastword $(sort $(wildcard target/debian/$(NAME)_*.deb)))

uninstall:
	sudo apt remove -y $(NAME)

clean:
	cargo clean
