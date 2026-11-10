const HELP: &str = "repo-notes-backend: WSL用ノートバックエンド\n\nCommands:\n  --stdio     JSON Linesで接続（最初にhelloが必要）\n  --version   アプリのバージョン\n  note read   ノートの読込\n  note write  保存元を比較した作成・更新\n\nExamples:\n  repo-notes-backend --stdio\n  repo-notes-backend note read --help\n  repo-notes-backend note write --help\n\n--stdioは標準入力のEOFまたはshutdown要求で終了します。stdoutは応答/イベント、診断はstderrへ出力します。";

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.as_slice() == ["--stdio"] {
        if let Err(error) = repo_notes_lib::backend::run_stdio() {
            eprintln!("{error}");
            std::process::exit(1);
        }
    } else if args.as_slice() == ["--version"] {
        println!("{}", repo_notes_lib::backend::APP_VERSION);
    } else if args.as_slice() == ["--help"] || args.as_slice() == ["-h"] {
        println!("{HELP}");
    } else if args.first().is_some_and(|arg| arg == "note") {
        std::process::exit(repo_notes_lib::run_cli(args).unwrap_or(2));
    } else {
        eprintln!("{HELP}");
        std::process::exit(2);
    }
}
