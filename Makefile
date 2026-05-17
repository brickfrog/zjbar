PLUGIN_DIR := $(HOME)/.config/zellij/plugins
LAYOUT_DIR := $(HOME)/.config/zellij/layouts
WASM       := target/wasm32-wasip1/release/zjbar.wasm
TAG        := $(shell git describe --tags --exact-match 2>/dev/null)
CUR_VER    := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')

# Portable in-place sed: GNU sed uses -i (no arg), BSD/macOS sed uses -i ''.
SED_I      := $(shell sed --version >/dev/null 2>&1 && echo 'sed -i' || echo 'sed -i ""')

.PHONY: build install install-layouts install-codex-hooks uninstall-codex-hooks install-gemini-hooks uninstall-gemini-hooks uninstall clean bump release

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

bump:
	@if [ -z "$(V)" ]; then echo "Usage: make bump V=x.y.z"; exit 1; fi
	@if [ "$(V)" = "$(CUR_VER)" ]; then echo "Error: version $(V) is the same as current"; exit 1; fi
	@if ! git diff --quiet || ! git diff --cached --quiet; then \
		echo "Error: working tree is dirty — commit or stash first"; exit 1; \
	fi
	@echo "Bumping $(CUR_VER) → $(V) ..."
	@# 1. Cargo.toml
	$(SED_I) 's/^version = "$(CUR_VER)"/version = "$(V)"/' Cargo.toml
	@# 2-3. README WASM download URLs
	$(SED_I) 's|releases/download/v$(CUR_VER)/zjbar.wasm|releases/download/v$(V)/zjbar.wasm|' README.md README.zh-CN.md
	@# 4. .claude-plugin/marketplace.json (both version fields)
	$(SED_I) 's/"$(CUR_VER)"/"$(V)"/g' .claude-plugin/marketplace.json
	@# 5. .claude-plugin/plugin.json
	$(SED_I) 's/"$(CUR_VER)"/"$(V)"/' .claude-plugin/plugin.json
	@# 6. opencode-plugin/package.json
	$(SED_I) 's/"version": "$(CUR_VER)"/"version": "$(V)"/' opencode-plugin/package.json
	@# 7. Build to update Cargo.lock
	cargo build --release
	@# Verify no stale references remain
	@if grep -rq 'releases/download/v$(CUR_VER)' README.md README.zh-CN.md; then \
		echo "Error: stale version $(CUR_VER) still found in README"; exit 1; \
	fi
	@# Commit and tag
	git add Cargo.toml Cargo.lock README.md README.zh-CN.md \
		.claude-plugin/marketplace.json .claude-plugin/plugin.json \
		opencode-plugin/package.json
	git commit -m "chore: bump version to v$(V)"
	git tag v$(V)
	@echo "Done. Run 'make release' to publish."

release: build
	@if [ -z "$(TAG)" ]; then \
		echo "Error: HEAD has no tag. Tag first with: git tag vX.Y.Z"; \
		exit 1; \
	fi
	git push origin main
	git push origin $(TAG)
	@PREV=$$(git describe --tags --abbrev=0 $(TAG)^ 2>/dev/null); \
	if [ -n "$$PREV" ]; then \
		NOTES=$$(printf '## What'\''s Changed\n\n'; \
			git log --pretty=format:'- %s' $$PREV..$(TAG); \
			printf '\n\n**Full Changelog**: https://github.com/brickfrog/zjbar/compare/%s...$(TAG)\n' "$$PREV"); \
	else \
		NOTES="Initial release"; \
	fi; \
	gh release create $(TAG) $(WASM) --title "$(TAG)" --notes "$$NOTES"
	cd opencode-plugin && npm publish
