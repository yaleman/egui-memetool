
.DEFAULT: help
.PHONY: help
help:
	@grep -E -h '\s##\s' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'


.PHONY: build
build: ## build the Tauri app
build:
	cargo tauri build

.PHONY: sign
sign: ## sign the built app
sign:
	codesign -s "${APPLE_SIGNING_IDENTITY}" --timestamp target/release/bundle/macos/memetool.app

.PHONY: check_sign
check_sign: ## Check the signature
check_sign:
	codesign -d -vvv target/release/bundle/macos/memetool.app
