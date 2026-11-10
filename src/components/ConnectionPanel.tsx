import { useState, type FormEvent } from "react";
import { useIsMutating } from "@tanstack/react-query";
import { useConnection } from "../lib/connection";
import { useDraftChanges } from "../lib/noteDrafts";
import type { WslConfig } from "../lib/connectionTypes";

function Settings({ initial, onClose }: { initial: WslConfig | null; onClose: () => void }) {
  const { view, connect } = useConnection();
  const { unsaved, discard } = useDraftChanges();
  const pending = useIsMutating();
  const [distro, setDistro] = useState(initial?.distro ?? view.distributions[0] ?? "Ubuntu");
  const [user, setUser] = useState(initial?.user ?? "");
  const [root, setRoot] = useState(initial?.dataRoot ?? "");
  const [error, setError] = useState<string | null>(null);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      await connect({ distro: distro.trim(), user: user.trim(), dataRoot: root.trim() || null });
      onClose();
    } catch (error) { setError(String(error)); }
    finally { setSubmitting(false); }
  };
  return <form className="connection-settings" onSubmit={(event) => void submit(event)}>
    <h2>WSL 接続設定</h2>
    <label>ディストリビューション<input value={distro} list="wsl-distributions" onChange={(event) => setDistro(event.target.value)} required /></label>
    <datalist id="wsl-distributions">{view.distributions.map((name) => <option key={name} value={name} />)}</datalist>
    <label>Linux ユーザー<input value={user} onChange={(event) => setUser(event.target.value)} required /></label>
    <label>ノート保存先<input value={root} onChange={(event) => setRoot(event.target.value)} placeholder="空欄: Linux ホームの .repo-notes" /></label>
    <p>エージェントと同じ保存先を指定してください。REPO_NOTES_DIR を使っている場合は、その Linux 絶対パスを入力します。</p>
    {unsaved.length > 0 && <div role="status">
      <p>未保存の下書き {unsaved.length} 件があります。別の接続先へ変える前に保存または破棄してください。</p>
      <ul>{unsaved.map((note) => <li key={`${note.repoKey}/${note.fileName}`}>{note.fileName}</li>)}</ul>
      {confirmDiscard ? <><span>すべての未保存の下書きを破棄します。</span><button type="button" className="btn danger" disabled={pending > 0} onClick={() => { discard(); setConfirmDiscard(false); }}>破棄する</button><button type="button" className="btn" onClick={() => setConfirmDiscard(false)}>取消</button></> : <button type="button" className="btn" onClick={() => setConfirmDiscard(true)}>下書きを破棄</button>}
    </div>}
    {error && <p className="error" role="alert">{error}</p>}
    <div className="connection-actions"><button className="btn primary" disabled={submitting || pending > 0}>{submitting ? "接続中…" : "保存して接続"}</button><button type="button" className="btn" onClick={onClose}>閉じる</button></div>
  </form>;
}

export function ConnectionPanel() {
  const { view, connect } = useConnection();
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (view.platform !== "windows") return view.message ? <div className="connection-bar error" role="alert">{view.message}</div> : null;
  const reconnect = async () => {
    if (!view.config) { setEditing(true); return; }
    setError(null);
    try { await connect(view.config); } catch (error) { setError(String(error)); }
  };
  return <section className="connection-panel" aria-label="WSL 接続">
    <div className="connection-bar" role="status">
      <strong>{view.state === "connected" ? "WSL 接続済み" : view.state === "connecting" ? "WSL 接続中…" : "WSL 未接続"}</strong>
      {view.config && <span>{view.config.distro} / {view.config.user}</span>}
      {view.dataRoot && <span>{view.dataRoot}</span>}
      <button type="button" className="btn" disabled={view.state === "connecting"} onClick={() => setEditing((current) => !current)}>接続設定</button>
      {view.state === "disconnected" && <button type="button" className="btn" onClick={() => void reconnect()}>再接続</button>}
    </div>
    {(view.message || error) && <p className="connection-message error" role="alert">{error ?? view.message}</p>}
    {view.state === "connected" && !view.externalAvailable && <p className="connection-message">外部操作には WSL の既定ユーザーと同じユーザーを選択してください。アプリ内の閲覧・編集は利用できます。</p>}
    {(editing || (!view.config && view.state !== "connecting")) && <Settings initial={view.config} onClose={() => setEditing(false)} />}
  </section>;
}
