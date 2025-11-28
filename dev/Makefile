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

# Colors for output (using tput for portability)
COLOR_RESET := $(shell tput sgr0)
COLOR_BOLD := $(shell tput bold)
COLOR_GREEN := $(shell tput setaf 2)
COLOR_YELLOW := $(shell tput setaf 3)
COLOR_BLUE := $(shell tput setaf 4)

# Scripts directory (relative to dev/)
SCRIPTS_DIR := scripts

##@ Development Commands

help: ## Display this help message
	@printf "%bAvailable targets:%b\n" "$(COLOR_BOLD)" "$(COLOR_RESET)"
	@printf "\n"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "; green="$(COLOR_GREEN)"; reset="$(COLOR_RESET)"}; {printf "  %s%-20s%s %s\n", green, $$1, reset, $$2}'
	@printf "\n"

fmt: ## Format all Rust code
	@printf "%bFormatting code...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	cargo fmt --all

clippy: ## Run clippy linter with all targets and features
	@printf "%bRunning clippy...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	cargo clippy --all-targets --all-features -- -D warnings

check: ## Check if code compiles without building
	@printf "%bChecking compilation...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	cargo check

test: ## Run all tests with single thread
	@printf "%bRunning tests...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	cargo test -- --test-threads=1

test-ignored: ## Run ignored tests
	@printf "%bRunning ignored tests...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	cargo test -- --ignored --test-threads=1

build: ## Build debug version
	@printf "%bBuilding debug version...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	cargo build

release: ## Build release version
	@printf "%bBuilding release version...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	cargo build --release

install: ## Install the binary (release mode)
	@printf "%bInstalling binary...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	cargo install --path .

##@ Quality Checks

pre-commit: fmt clippy check test ## Run all pre-commit checks (fmt, clippy, check, test)
	@printf "%b✓ All pre-commit checks passed!%b\n" "$(COLOR_GREEN)" "$(COLOR_RESET)"

all: pre-commit ## Alias for pre-commit

##@ Analysis Scripts

cognitive: ## Run clippy cognitive complexity analysis
	@printf "%bRunning cognitive complexity analysis...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/clippy_cognitive_complexity.sh

errors: ## Analyze and report clippy errors
	@printf "%bAnalyzing clippy errors...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/clippy_errors.sh

complexity: ## Generate code complexity report
	@printf "%bGenerating complexity report...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/complexity_report.sh

##@ Documentation & Diagrams

docs: ## Generate Rust documentation
	@printf "%bGenerating documentation...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/generate_docs.sh

docs-open: docs ## Generate and open documentation in browser
	@printf "%bOpening documentation...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	cargo doc --open

module-structure: ## Generate module structure diagrams
	@printf "%bGenerating module structure...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/module_structure.sh

controlflow: ## Generate control flow diagram
	@printf "%bGenerating control flow diagram...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	@bash $(SCRIPTS_DIR)/generate_controlflow_diagram.sh

##@ Cleanup

clean: ## Remove build artifacts
	@printf "%bCleaning build artifacts...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	cargo clean

clean-all: clean ## Remove all build artifacts and generated files
	@printf "%bCleaning all generated files...%b\n" "$(COLOR_BLUE)" "$(COLOR_RESET)"
	@rm -rf target/
	@rm -f $(SCRIPTS_DIR)/*.txt
	@rm -rf $(SCRIPTS_DIR)/Modules/

##@ Combined Targets

quality: cognitive errors complexity ## Run all quality analysis scripts
	@printf "%b✓ Quality analysis complete!%b\n" "$(COLOR_GREEN)" "$(COLOR_RESET)"

analysis: quality ## Alias for quality

full-check: pre-commit quality ## Run all checks and quality analysis
	@printf "\n"
	@printf "%b═══════════════════════════════════════════════════════════════%b\n" "$(COLOR_BOLD)" "$(COLOR_RESET)"
	@printf "%b                    Full Check Summary%b\n" "$(COLOR_BOLD)" "$(COLOR_RESET)"
	@printf "%b═══════════════════════════════════════════════════════════════%b\n" "$(COLOR_BOLD)" "$(COLOR_RESET)"
	@printf "\n"
	@printf "%bPre-commit Checks:%b\n" "$(COLOR_BOLD)" "$(COLOR_RESET)"
	@printf "  %b✓%b Formatting (cargo fmt --all)\n" "$(COLOR_GREEN)" "$(COLOR_RESET)"
	@printf "  %b✓%b Linting (cargo clippy --all-targets --all-features)\n" "$(COLOR_GREEN)" "$(COLOR_RESET)"
	@printf "  %b✓%b Compilation (cargo check)\n" "$(COLOR_GREEN)" "$(COLOR_RESET)"
	@printf "  %b✓%b Tests (cargo test -- --test-threads=1)\n" "$(COLOR_GREEN)" "$(COLOR_RESET)"
	@printf "\n"
	@printf "%bQuality Analysis:%b\n" "$(COLOR_BOLD)" "$(COLOR_RESET)"
	@printf "  %b✓%b Cognitive Complexity Analysis\n" "$(COLOR_GREEN)" "$(COLOR_RESET)"
	@printf "  %b✓%b Clippy Errors Analysis\n" "$(COLOR_GREEN)" "$(COLOR_RESET)"
	@printf "  %b✓%b Code Complexity Report\n" "$(COLOR_GREEN)" "$(COLOR_RESET)"
	@printf "\n"
	@printf "%b═══════════════════════════════════════════════════════════════%b\n" "$(COLOR_BOLD)" "$(COLOR_RESET)"
	@printf "%b✓ All checks passed successfully!%b\n" "$(COLOR_GREEN)" "$(COLOR_RESET)"
	@printf "%b═══════════════════════════════════════════════════════════════%b\n" "$(COLOR_BOLD)" "$(COLOR_RESET)"
	@printf "\n"

