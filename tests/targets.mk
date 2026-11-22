.PHONY: test check test-integration
test:
	$(rust) test --workspace --locked -- --test-threads=1
	node --test tests/extension/*.test.mjs
	gjs -m tests/extension/stills.test.js
check: format-check lint test
test-integration: build
	bash tests/session/test-session.sh $(SESSION)

.PHONY: test-controller test-renderer
test-controller: build
	/usr/bin/python3 tests/session/test_x11_resize.py
	UBRUS_TEST_COMPONENT=controller bash tests/session/test-session.sh x11
test-renderer: build
	UBRUS_TEST_COMPONENT=renderer bash tests/session/test-session.sh x11

.PHONY: test-update
test-update: deb
	UBRUS_TEST_COMPONENT=update bash tests/session/test-session.sh x11
