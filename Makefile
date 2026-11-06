# Flight Replay build and check automation. Run `make help` to list the targets.

CARGO   ?= cargo
NAME    := flight-replay
APP_PKG := flight-replay

# Extra arguments for the run target: make run ARGS="--some-flag"
ARGS ?=

.DEFAULT_GOAL := build

.PHONY: help build release run test fmt fmt-check lint deny check install clean golden fixtures

help: ## Show this help
	@awk 'BEGIN {FS = ":.*## "} /^[a-z-]+:.*## / {printf "  %-12s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Debug build of all crates
	$(CARGO) build --workspace

release: ## Optimised build of the application
	$(CARGO) build --release -p $(APP_PKG)

run: ## Run the application (extra arguments in ARGS)
	$(CARGO) run -p $(APP_PKG) -- $(ARGS)

test: ## Unit and integration tests
	$(CARGO) test --workspace

fmt: ## Format the code with rustfmt
	$(CARGO) fmt --all

fmt-check: ## Fail if any file is not formatted
	$(CARGO) fmt --all -- --check

lint: ## Clippy, with warnings as errors
	$(CARGO) clippy --workspace --all-targets -- -D warnings

deny: ## Licenses, advisories, sources and duplicate crates (deny.toml)
	$(CARGO) deny check

check: fmt-check lint test deny ## All checks; run before every commit

install: ## Install the application with cargo
	$(CARGO) install --path crates/gui

golden: ## Rewrite the golden images of the overlay tests (review the result before committing)
	FLIGHT_REPLAY_UPDATE_GOLDEN=1 $(CARGO) test -p flight-replay-core --test overlay_golden

# The recording that fixtures are cut from stays outside the repository: make fixtures SAMPLES=/path RECORDING=AvatarG0153
SAMPLES   ?= $(FLIGHT_REPLAY_SAMPLES)
RECORDING ?= AvatarG0153

fixtures: ## Cut the test fixture from a local recording and move its GPS to open ocean
	@test -n "$(SAMPLES)" || { echo "set SAMPLES to the folder of the recording"; exit 1; }
	$(CARGO) xtask fixtures --source "$(SAMPLES)" --recording $(RECORDING)

clean: ## Remove build output
	$(CARGO) clean
