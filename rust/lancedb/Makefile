#
# Makefile for running tests.
#

# Run all tests.
all-tests: feature-tests

# Run tests for every feature. This requires using docker compose to set up
# the environment.
feature-tests:
	../../ci/run_with_docker_compose.sh \
		cargo test --all-features --tests --locked --examples $(CARGO_ARGS)
.PHONY: feature-tests
