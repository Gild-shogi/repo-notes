CLAUDE_SKILL_DIR := $(HOME)/.claude/skills/repo-notes
AGENTS_SKILL_DIR := $(HOME)/.agents/skills/repo-notes

APP_NAME := repo-notes
APP_BUNDLE := src-tauri/target/release/bundle/macos/$(APP_NAME).app
INSTALL_DIR := /Applications
APP_BINARY ?= src-tauri/target/release/$(APP_NAME)
BUILD_CLI ?= pnpm tauri build --no-bundle

.PHONY: install-skill test release

# 起動中のアプリを終了 → リリースビルド → /Applications へ再配置 → 起動
release:
	-osascript -e 'tell application "$(APP_NAME)" to quit' 2>/dev/null
	-pkill -f "$(APP_NAME).app/Contents/MacOS/$(APP_NAME)" 2>/dev/null
	sleep 1
	pnpm tauri build --bundles app
	rm -rf "$(INSTALL_DIR)/$(APP_NAME).app"
	cp -R "$(APP_BUNDLE)" "$(INSTALL_DIR)/"
	open "$(INSTALL_DIR)/$(APP_NAME).app"
	@echo "released: $(INSTALL_DIR)/$(APP_NAME).app"

# skill/ を Claude Code と Codex 両方のスキルディレクトリにコピーする
install-skill:
	@if [ "$(YES)" != "1" ]; then \
		printf '実行内容:\n  1. アプリバイナリのビルド\n  2. スキルと同じバイナリを %s と %s へコピー\n' "$(CLAUDE_SKILL_DIR)" "$(AGENTS_SKILL_DIR)"; \
		[ -t 0 ] || { echo '非対話では make YES=1 install-skill を指定してください'; exit 1; }; \
		printf '実行する場合は yes と入力してください [yes/N]: '; \
		read -r answer; \
		[ "$$answer" = "yes" ] || { echo '中止しました'; exit 1; }; \
	fi
	$(BUILD_CLI)
	test -x "$(APP_BINARY)"
	mkdir -p "$(CLAUDE_SKILL_DIR)/bin" "$(AGENTS_SKILL_DIR)/bin"
	cp -R skill/. "$(CLAUDE_SKILL_DIR)/"
	cp -R skill/. "$(AGENTS_SKILL_DIR)/"
	cp "$(APP_BINARY)" "$(CLAUDE_SKILL_DIR)/bin/$(APP_NAME)"
	cp "$(APP_BINARY)" "$(AGENTS_SKILL_DIR)/bin/$(APP_NAME)"
	@echo "installed: $(CLAUDE_SKILL_DIR)"
	@echo "installed: $(AGENTS_SKILL_DIR)"

test:
	cargo test --manifest-path src-tauri/Cargo.toml
	bash tests/skill/test-resolve-note-dir.sh
	bash tests/skill/test-session-start-notes.sh
	pnpm test
