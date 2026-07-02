CLAUDE_SKILL_DIR := $(HOME)/.claude/skills/repo-notes
AGENTS_SKILL_DIR := $(HOME)/.agents/skills/repo-notes

.PHONY: install-skill test

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
	pnpm test
