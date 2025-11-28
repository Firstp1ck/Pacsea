# Makefile for Pacsea
# Provides convenient shortcuts for common development tasks
#
# Note: This Makefile is located in dev/ directory.
# Run from project root with: make -C dev/ <target>
# Or use the wrapper Makefile in the project root.

.PHONY: help fmt clippy check test build release clean
.PHONY: pre-commit all
.PHONY: cognitive errors complexity
.PHONY: docs module-structure controlflow
.PHONY: install

# Default target
.DEFAULT_GOAL := help

# Colors for output
COLOR_RESET := \033[0m
COLOR_BOLD := \033[1m
COLOR_GREEN := \033[32m
COLOR_YELLOW := \033[33m
COLOR_BLUE := \033[34m

# Scripts directory (relative to dev/)
SCRIPTS_DIR := scripts

##@ Development Commands

help: ## Display this help message
	@echo "$(COLOR_BOLD)Available targets:$(COLOR_RESET)"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  $(COLOR_GREEN)%-20s$(COLOR_RESET) %s\n", $$1, $$2}'
	@echo ""

fmt: ## Format all Rust code
	@echo "$(COLOR_BLUE)Formatting code...$(COLOR_RESET)"
	cargo fmt --all

clippy: ## Run clippy linter with all targets and features
	@echo "$(COLOR_BLUE)Running clippy...$(COLOR_RESET)"
	cargo clippy --all-targets --all-features -- -D warnings

check: ## Check if code compiles without building
	@echo "$(COLOR_BLUE)Checking compilation...$(COLOR_RESET)"
	cargo check

test: ## Run all tests with single thread
	@echo "$(COLOR_BLUE)Running tests...$(COLOR_RESET)"
	cargo test -- --test-threads=1

test-ignored: ## Run ignored tests
	@echo "$(COLOR_BLUE)Running ignored tests...$(COLOR_RESET)"
	cargo test -- --ignored --test-threads=1

build: ## Build debug version
	@echo "$(COLOR_BLUE)Building debug version...$(COLOR_RESET)"
	cargo build

release: ## Build release version
	@echo "$(COLOR_BLUE)Building release version...$(COLOR_RESET)"
	cargo build --release

install: ## Install the binary (release mode)
	@echo "$(COLOR_BLUE)Installing binary...$(COLOR_RESET)"
	cargo install --path .

##@ Quality Checks

pre-commit: fmt clippy check test ## Run all pre-commit checks (fmt, clippy, check, test)
	@echo "$(COLOR_GREEN)✓ All pre-commit checks passed!$(COLOR_RESET)"

all: pre-commit ## Alias for pre-commit

##@ Analysis Scripts

cognitive: ## Run clippy cognitive complexity analysis
	@echo "$(COLOR_BLUE)Running cognitive complexity analysis...$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/clippy_cognitive_complexity.sh

errors: ## Analyze and report clippy errors
	@echo "$(COLOR_BLUE)Analyzing clippy errors...$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/clippy_errors.sh

complexity: ## Generate code complexity report
	@echo "$(COLOR_BLUE)Generating complexity report...$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/complexity_report.sh

##@ Documentation & Diagrams

docs: ## Generate Rust documentation
	@echo "$(COLOR_BLUE)Generating documentation...$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/generate_docs.sh

docs-open: docs ## Generate and open documentation in browser
	@echo "$(COLOR_BLUE)Opening documentation...$(COLOR_RESET)"
	cargo doc --open

module-structure: ## Generate module structure diagrams
	@echo "$(COLOR_BLUE)Generating module structure...$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/module_structure.sh

controlflow: ## Generate control flow diagram
	@echo "$(COLOR_BLUE)Generating control flow diagram...$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/generate_controlflow_diagram.sh

##@ Cleanup

clean: ## Remove build artifacts
	@echo "$(COLOR_BLUE)Cleaning build artifacts...$(COLOR_RESET)"
	cargo clean

clean-all: clean ## Remove all build artifacts and generated files
	@echo "$(COLOR_BLUE)Cleaning all generated files...$(COLOR_RESET)"
	@rm -rf target/
	@rm -f $(SCRIPTS_DIR)/*.txt
	@rm -rf $(SCRIPTS_DIR)/Modules/

##@ Combined Targets

quality: cognitive errors complexity ## Run all quality analysis scripts
	@echo "$(COLOR_GREEN)✓ Quality analysis complete!$(COLOR_RESET)"

analysis: quality ## Alias for quality

full-check: pre-commit quality ## Run all checks and quality analysis
	@echo "$(COLOR_GREEN)✓ Full check complete!$(COLOR_RESET)"

