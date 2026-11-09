use crate::{
    model::SaveError,
    store::{self, DataRoot},
};
use std::{
    collections::HashMap,
    ffi::OsString,
    io::{self, IsTerminal, Read, Write},
    path::Path,
};

const HELP: &str = "repo-notes: 引数なしではデスクトップアプリを起動します。\n\nCommands:\n  note read   保存済みノートの読込\n  note write  保存元を比較したノートの作成・更新\n\nExamples:\n  repo-notes note read --help\n  repo-notes note write --help";
const READ_HELP: &str = "repo-notes note read --repo-key KEY --file-name NAME [--output json|raw]\n\nExamples:\n  repo-notes note read --repo-key owner-repo--12345678 --file-name 2026-11-10-note.md --output raw > snapshot.md\n\nraw は末尾の改行を付加せず元の内容を返します。";
const WRITE_HELP: &str = "repo-notes note write --repo-key KEY --file-name NAME --content-file PATH|- (--expected-file PATH | --new) [--dry-run]\n\nExamples:\n  repo-notes note write --repo-key owner-repo--12345678 --file-name 2026-11-10-note.md --expected-file snapshot.md --content-file draft.md\n  repo-notes note write --repo-key owner-repo--12345678 --file-name 2026-11-10-new.html --new --content-file draft.html --dry-run\n\nExit codes: 0 成功 / 1 I/O等の失敗 / 2 引数不正 / 3 保存競合\n競合時はsnapshotとdraftを残して最新を読み直してください。強制上書きはありません。";

pub fn run(args: Vec<OsString>) -> Option<i32> {
    if args.is_empty() || args[0].to_string_lossy().starts_with("-psn_") {
        return None;
    }
    if args[0] == "--help" || args[0] == "-h" {
        println!("{HELP}");
        return Some(0);
    }
    if args[0] == "note"
        && args
            .get(1)
            .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!("{HELP}");
        return Some(0);
    }
    Some(match execute(args) {
        Ok(()) => 0,
        Err((code, error)) => {
            eprintln!("{}", serde_json::to_string(&error).unwrap());
            code
        }
    })
}

fn execute(args: Vec<OsString>) -> Result<(), (i32, SaveError)> {
    if args.first().is_none_or(|arg| arg != "note") {
        return Err(usage(HELP));
    }
    let operation = args
        .get(1)
        .and_then(|arg| arg.to_str())
        .ok_or_else(|| usage(HELP))?
        .to_string();
    let help = match operation.as_str() {
        "read" => READ_HELP,
        "write" => WRITE_HELP,
        _ => return Err(usage(HELP)),
    };
    if args[2..].iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{help}");
        return Ok(());
    }
    let mut options = HashMap::new();
    let mut args = args.into_iter().skip(2);
    while let Some(flag) = args.next() {
        let flag = flag.to_str().ok_or_else(|| usage(help))?.to_string();
        let allowed = match operation.as_str() {
            "read" => ["--repo-key", "--file-name", "--output"].contains(&flag.as_str()),
            _ => [
                "--repo-key",
                "--file-name",
                "--content-file",
                "--expected-file",
                "--new",
                "--dry-run",
            ]
            .contains(&flag.as_str()),
        };
        if !allowed || options.contains_key(&flag) {
            return Err(usage(help));
        }
        let value = if ["--new", "--dry-run"].contains(&flag.as_str()) {
            "true".into()
        } else {
            let value = args.next().ok_or_else(|| usage(help))?;
            if value.to_string_lossy().starts_with("--") {
                return Err(usage(help));
            }
            value
        };
        options.insert(flag, value);
    }
    let required = |key: &str| {
        options
            .get(key)
            .and_then(|value| value.to_str())
            .ok_or_else(|| usage(help))
    };
    let repo_key = required("--repo-key")?;
    let file_name = required("--file-name")?;
    if operation == "read" {
        let output = options
            .get("--output")
            .and_then(|value| value.to_str())
            .unwrap_or("json");
        if !["raw", "json"].contains(&output) {
            return Err(usage(help));
        }
        let root = DataRoot::open_existing(&store::data_root()).map_err(failure)?;
        let note = store::read_note(&root, repo_key, file_name).map_err(failure)?;
        if output == "raw" {
            io::stdout()
                .write_all(note.raw.as_bytes())
                .map_err(|e| failure(e.to_string()))?;
        } else {
            println!(
                "{}",
                serde_json::to_string(&note).map_err(|e| failure(e.to_string()))?
            );
        }
        return Ok(());
    }
    if options.contains_key("--new") == options.contains_key("--expected-file") {
        return Err(usage(help));
    }
    let input = options.get("--content-file").ok_or_else(|| usage(help))?;
    let root = DataRoot::open_existing(&store::data_root()).map_err(failure)?;
    let content = if input == "-" {
        if io::stdin().is_terminal() {
            return Err(usage(WRITE_HELP));
        }
        let mut content = String::new();
        io::stdin()
            .read_to_string(&mut content)
            .map_err(|e| failure(e.to_string()))?;
        content
    } else {
        std::fs::read_to_string(Path::new(input)).map_err(|e| failure(e.to_string()))?
    };
    let expected = options
        .get("--expected-file")
        .map(|path| std::fs::read_to_string(Path::new(path)))
        .transpose()
        .map_err(|e| failure(e.to_string()))?;
    let result = if options.contains_key("--dry-run") {
        store::check_note_save(&root, repo_key, file_name, expected.as_deref())
            .map(|()| serde_json::json!({"dryRun":true, "fileName":file_name}))
    } else {
        store::save_note(&root, repo_key, file_name, expected.as_deref(), &content)
            .map(|outcome| serde_json::json!({"fileName":file_name,"cleanupWarnings":outcome.cleanup_warnings}))
    };
    let result = result.map_err(|error| {
        (
            if matches!(error, SaveError::Conflict) {
                3
            } else {
                1
            },
            error,
        )
    })?;
    println!("{result}");
    Ok(())
}

fn failure(message: String) -> (i32, SaveError) {
    (1, SaveError::Failure(message))
}
fn usage(help: &str) -> (i32, SaveError) {
    (2, SaveError::Failure(format!("引数が不正です。\n{help}")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn incomplete_headless_write_fails_without_launching_the_gui() {
        assert_eq!(super::run(vec!["note".into(), "write".into()]), Some(2));
    }
}
