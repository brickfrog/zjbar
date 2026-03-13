PLUGIN_DIR := $(HOME)/.config/zellij/plugins
LAYOUT_DIR := $(HOME)/.config/zellij/layouts
WASM       := target/wasm32-wasip1/release/zjbar.wasm
TAG        := $(shell git describe --tags --exact-match 2>/dev/null)

.PHONY: build install install-layouts install-codex-hooks uninstall-codex-hooks install-gemini-hooks uninstall-gemini-hooks setup-gemini-extension unlink-gemini-extension uninstall clean release

build:
	cargo build --release
	@mkdir -p $(PLUGIN_DIR)
	cp $(WASM) $(PLUGIN_DIR)/zjbar.wasm

install-layouts:
	@mkdir -p $(LAYOUT_DIR)
	cp layout.kdl $(LAYOUT_DIR)/zjbar.kdl
	cp layout.swap.kdl $(LAYOUT_DIR)/zjbar.swap.kdl

install-codex-hooks:
	scripts/install-codex-hooks.sh

uninstall-codex-hooks:
	scripts/install-codex-hooks.sh --uninstall

install-gemini-hooks:
	scripts/install-gemini-hooks.sh

uninstall-gemini-hooks:
	scripts/install-gemini-hooks.sh --uninstall

setup-gemini-extension:
	scripts/setup-gemini-extension.sh link

unlink-gemini-extension:
	scripts/setup-gemini-extension.sh unlink

install: build install-layouts
	@echo "Installed plugin and layouts."

uninstall:
	rm -f $(PLUGIN_DIR)/zjbar.wasm
	rm -f $(LAYOUT_DIR)/zjbar.kdl $(LAYOUT_DIR)/zjbar.swap.kdl
	-scripts/install-codex-hooks.sh --uninstall 2>/dev/null
	-scripts/install-gemini-hooks.sh --uninstall 2>/dev/null
	@echo "Uninstalled."

clean:
	cargo clean

release: build
	@if [ -z "$(TAG)" ]; then \
		echo "Error: HEAD has no tag. Tag first with: git tag vX.Y.Z"; \
		exit 1; \
	fi
	git push origin $(TAG)
	@PREV=$$(git describe --tags --abbrev=0 $(TAG)^ 2>/dev/null); \
	if [ -n "$$PREV" ]; then \
		NOTES=$$(printf '## What'\''s Changed\n\n'; \
			git log --pretty=format:'- %s' $$PREV..$(TAG); \
			printf '\n\n**Full Changelog**: https://github.com/imroc/zjbar/compare/%s...$(TAG)\n' "$$PREV"); \
	else \
		NOTES="Initial release"; \
	fi; \
	gh release create $(TAG) $(WASM) --title "$(TAG)" --notes "$$NOTES"
	cd opencode-plugin && npm publish
