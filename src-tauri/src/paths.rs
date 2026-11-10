use std::path::{Path, PathBuf};

/// 実行系のファイルはノート内リンクから開かせない（openで即実行され得るもの）
const BLOCKED_EXTENSIONS: &[&str] = &[
    "app",
    "command",
    "terminal",
    "workflow",
    "scpt",
    "applescript",
    "jar",
    "pkg",
    "dmg",
];

/// ノート内リンクから開いてよいパスかを検証し、正規化済みパスを返す。
/// - 実在すること（canonicalize で symlink / `..` を解決）
/// - 解決後が home 配下であること
/// - 実行系拡張子でないこと
pub fn validate_openable(path: &str, home: &Path) -> Result<PathBuf, String> {
    let canonical = Path::new(path)
        .canonicalize()
        .map_err(|_| format!("ファイルが見つかりません: {path}"))?;
    let canonical_home = home
        .canonicalize()
        .map_err(|e| format!("ホームディレクトリを解決できません: {e}"))?;
    if !canonical.starts_with(&canonical_home) {
        return Err(format!(
            "ホームディレクトリ外のパスは開けません: {}",
            canonical.display()
        ));
    }
    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if BLOCKED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!("このファイル形式はリンクから開けません: .{ext}"));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_outside_home_and_blocked_extensions() {
        let home = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();

        let ok_file = home.path().join("doc.md");
        fs::write(&ok_file, "x").unwrap();
        assert!(validate_openable(ok_file.to_str().unwrap(), home.path()).is_ok());

        let out_file = outside.path().join("doc.md");
        fs::write(&out_file, "x").unwrap();
        assert!(validate_openable(out_file.to_str().unwrap(), home.path()).is_err());

        let sneaky = format!("{}/../{}", home.path().display(), out_file.display());
        assert!(validate_openable(&sneaky, home.path()).is_err());

        #[cfg(unix)]
        {
            let link = home.path().join("link.md");
            std::os::unix::fs::symlink(&out_file, &link).unwrap();
            assert!(validate_openable(link.to_str().unwrap(), home.path()).is_err());
        }

        let cmd = home.path().join("evil.command");
        fs::write(&cmd, "x").unwrap();
        assert!(validate_openable(cmd.to_str().unwrap(), home.path()).is_err());

        assert!(
            validate_openable(home.path().join("nope.md").to_str().unwrap(), home.path()).is_err()
        );
    }
}
