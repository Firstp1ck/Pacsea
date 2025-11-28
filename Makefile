# Wrapper Makefile - delegates to dev/Makefile
# This allows running 'make' from the project root

.PHONY: help

.DEFAULT_GOAL := help

# Delegate all targets to dev/Makefile
%:
	@$(MAKE) -C dev $@

help:
	@echo "This Makefile delegates to dev/Makefile"
	@echo "Run 'make -C dev help' or see dev/Makefile for available targets"
	@echo ""
	@$(MAKE) -C dev help
