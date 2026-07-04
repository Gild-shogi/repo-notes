CLAUDE_SKILL_DIR := $(HOME)/.claude/skills/repo-notes
AGENTS_SKILL_DIR := $(HOME)/.agents/skills/repo-notes

APP_NAME := repo-notes
APP_BUNDLE := src-tauri/target/release/bundle/macos/$(APP_NAME).app
INSTALL_DIR := /Applications

.PHONY: install-skill test release

# 起動中のアプリを終了 → リリースビルド → /Applications へ再配置 → 起動
release:
	-osascript -e 'tell application "$(APP_NAME)" to quit' 2>/dev/null
	-pkill -f "$(APP_NAME).app/Contents/MacOS/$(APP_NAME)" 2>/dev/null
	sleep 1
	pnpm tauri build
	rm -rf "$(INSTALL_DIR)/$(APP_NAME).app"
	cp -R "$(APP_BUNDLE)" "$(INSTALL_DIR)/"
	open "$(INSTALL_DIR)/$(APP_NAME).app"
	@echo "released: $(INSTALL_DIR)/$(APP_NAME).app"

# skill/ を Claude Code と Codex 両方のスキルディレクトリにコピーする
install-skill:
	mkdir -p "$(CLAUDE_SKILL_DIR)" "$(AGENTS_SKILL_DIR)"
	cp -R skill/. "$(CLAUDE_SKILL_DIR)/"
	cp -R skill/. "$(AGENTS_SKILL_DIR)/"
	@echo "installed: $(CLAUDE_SKILL_DIR)"
	@echo "installed: $(AGENTS_SKILL_DIR)"

test:
	cargo test --manifest-path src-tauri/Cargo.toml
	bash tests/skill/test-resolve-note-dir.sh
	bash tests/skill/test-session-start-notes.sh
	pnpm test
