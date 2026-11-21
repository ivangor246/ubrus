.PHONY: test check test-integration
test:
	$(rust) test --workspace --locked -- --test-threads=1
	node --test tests/extension/*.test.mjs
	gjs -m tests/extension/stills.test.js
check: format-check lint test
test-integration: build
	bash tests/session/test-session.sh $(SESSION)
