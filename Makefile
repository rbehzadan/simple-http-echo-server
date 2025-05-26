APP_NAME := $(shell sed -n 's/^name *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
VERSION  ?= $(shell sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
TAG := v$(VERSION)
BINARY := target/release/$(APP_NAME)

.PHONY: $(BINARY) all run tag clean

$(BINARY): Cargo.toml src/*.rs
	@echo "Building $(APP_NAME)..."
	cargo build --release

run: $(BINARY)
	@./$(BINARY)

all: $(TARGETS)
	@echo ""
	@echo "✓ All builds completed successfully!"

tag:
	@echo "Tagging version $(TAG)..."
	@git tag $(TAG)
	@git push origin $(TAG)

clean:
	@echo "Cleaning build artifacts..."
	cargo clean

