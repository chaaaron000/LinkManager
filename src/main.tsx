import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  CheckCircle2,
  FolderInput,
  FolderOpen,
  Link2,
  RefreshCw,
  Search,
  Shield,
  Trash2,
  XCircle,
} from "lucide-react";
import "./styles.css";

type LinkStatus =
  | "Ok"
  | "MissingTarget"
  | "MissingLink"
  | "NotSymlink"
  | "WrongTarget"
  | "AccessDenied"
  | "UnknownError";

type ManagedLink = {
  id: string;
  name: string;
  original_path: string;
  target_path: string;
  storage_root: string | null;
  kind: "DirectorySymlink";
  status: LinkStatus;
  created_at: number;
  last_checked_at: number | null;
};

type AppStateSnapshot = {
  links: ManagedLink[];
  is_admin: boolean;
};

type ReplacePreview = {
  original_path: string;
  storage_root: string;
  target_path: string;
  target_exists: boolean;
  original_exists: boolean;
  original_is_symlink: boolean;
  is_admin: boolean;
};

type ScanResult = {
  original_path: string;
  target_path: string;
  already_managed: boolean;
};

type ScanProgress = {
  current_path: string;
  scanned_count: number;
  found_count: number;
  done: boolean;
};

type TreeNode = {
  label: string;
  path: string;
  children: TreeNode[];
  link?: ManagedLink;
};

function pathParts(path: string): string[] {
  const normalized = path.replace(/\//g, "\\");
  const [drive, ...rest] = normalized.split("\\").filter(Boolean);
  return drive?.endsWith(":") ? [drive, ...rest] : rest.length ? [drive, ...rest] : [path];
}

function buildTree(links: ManagedLink[]): TreeNode[] {
  const roots: TreeNode[] = [];

  for (const link of links) {
    const parts = pathParts(link.original_path);
    let layer = roots;
    let acc = "";

    for (const [index, part] of parts.entries()) {
      acc = index === 0 ? part : `${acc}\\${part}`;
      let node = layer.find((candidate) => candidate.label === part);
      if (!node) {
        node = { label: part, path: acc, children: [] };
        layer.push(node);
      }
      if (index === parts.length - 1) {
        node.link = link;
      }
      layer = node.children;
    }
  }

  return roots.sort((a, b) => a.label.localeCompare(b.label));
}

function statusLabel(status: LinkStatus): string {
  switch (status) {
    case "Ok":
      return "정상";
    case "MissingTarget":
      return "대상 없음";
    case "MissingLink":
      return "링크 없음";
    case "NotSymlink":
      return "링크 아님";
    case "WrongTarget":
      return "대상 다름";
    case "AccessDenied":
      return "권한 없음";
    default:
      return "알 수 없음";
  }
}

function StatusBadge({ status }: { status: LinkStatus }) {
  const ok = status === "Ok";
  return (
    <span className={`status ${ok ? "ok" : "warn"}`}>
      {ok ? <CheckCircle2 size={14} /> : <AlertTriangle size={14} />}
      {statusLabel(status)}
    </span>
  );
}

function TreeView({
  nodes,
  selectedId,
  onSelect,
}: {
  nodes: TreeNode[];
  selectedId?: string;
  onSelect: (link: ManagedLink) => void;
}) {
  return (
    <div className="tree">
      {nodes.map((node) => (
        <TreeNodeView key={node.path} node={node} selectedId={selectedId} onSelect={onSelect} />
      ))}
    </div>
  );
}

function TreeNodeView({
  node,
  selectedId,
  onSelect,
  depth = 0,
}: {
  node: TreeNode;
  selectedId?: string;
  onSelect: (link: ManagedLink) => void;
  depth?: number;
}) {
  const selected = node.link?.id === selectedId;
  return (
    <div>
      <button
        className={`tree-row ${selected ? "selected" : ""}`}
        style={{ paddingLeft: 12 + depth * 16 }}
        onClick={() => node.link && onSelect(node.link)}
        disabled={!node.link}
        title={node.link?.target_path ?? node.path}
      >
        {node.link ? <Link2 size={15} /> : <FolderOpen size={15} />}
        <span>{node.label}</span>
        {node.link ? <span className={`dot ${node.link.status === "Ok" ? "ok" : "warn"}`} /> : null}
      </button>
      {node.children.map((child) => (
        <TreeNodeView
          key={child.path}
          node={child}
          selectedId={selectedId}
          onSelect={onSelect}
          depth={depth + 1}
        />
      ))}
    </div>
  );
}

function App() {
  const [links, setLinks] = React.useState<ManagedLink[]>([]);
  const [isAdmin, setIsAdmin] = React.useState(false);
  const [selectedId, setSelectedId] = React.useState<string>();
  const [message, setMessage] = React.useState("");
  const [replaceOpen, setReplaceOpen] = React.useState(false);
  const [importOpen, setImportOpen] = React.useState(false);

  const selected = links.find((link) => link.id === selectedId) ?? links[0];
  const tree = React.useMemo(() => buildTree(links), [links]);

  async function refresh() {
    const state = await invoke<AppStateSnapshot>("get_state");
    setLinks(state.links);
    setIsAdmin(state.is_admin);
    if (!selectedId && state.links[0]) {
      setSelectedId(state.links[0].id);
    }
  }

  async function runAction(action: () => Promise<void>, success?: string) {
    try {
      setMessage("");
      await action();
      if (success) setMessage(success);
    } catch (error) {
      setMessage(String(error));
    }
  }

  React.useEffect(() => {
    refresh().catch((error) => setMessage(String(error)));
  }, []);

  return (
    <main>
      <header>
        <div>
          <h1>Link Manager</h1>
          <p>원래 경로를 보존해서 폴더 심볼릭 링크를 관리합니다.</p>
        </div>
        <div className={`admin ${isAdmin ? "on" : ""}`}>
          <Shield size={16} />
          {isAdmin ? "관리자 권한" : "일반 권한"}
        </div>
      </header>

      <nav className="toolbar">
        <button onClick={() => setReplaceOpen(true)} disabled={!isAdmin} title="폴더를 이동하고 심볼릭 링크로 대체">
          <FolderInput size={17} />
          폴더 대체
        </button>
        <button onClick={() => setImportOpen(true)} title="기존 심볼릭 링크를 관리 목록에 추가">
          <Link2 size={17} />
          기존 링크 추가
        </button>
        <button onClick={() => refresh()} title="상태 새로고침">
          <RefreshCw size={17} />
          전체 검증
        </button>
        <button
          onClick={() => runAction(() => invoke("relaunch_as_admin"), "관리자 권한 실행을 요청했습니다.")}
          title="UAC로 관리자 권한 재실행"
        >
          <Shield size={17} />
          관리자 재실행
        </button>
      </nav>

      {message ? <div className="notice">{message}</div> : null}

      <section className="layout">
        <aside>
          <div className="pane-title">원래 경로 트리</div>
          {tree.length ? (
            <TreeView nodes={tree} selectedId={selected?.id} onSelect={(link) => setSelectedId(link.id)} />
          ) : (
            <div className="empty">관리 중인 링크가 없습니다.</div>
          )}
        </aside>

        <section className="detail">
          {selected ? (
            <>
              <div className="detail-head">
                <div>
                  <h2>{selected.name}</h2>
                  <StatusBadge status={selected.status} />
                </div>
              </div>
              <Field label="원래 경로" value={selected.original_path} />
              <Field label="실제 대상 경로" value={selected.target_path} />
              <Field label="보관 루트" value={selected.storage_root ?? "가져온 링크"} />
              <Field label="마지막 검증" value={selected.last_checked_at ? new Date(selected.last_checked_at * 1000).toLocaleString() : "-"} />

              <div className="actions">
                <button onClick={() => runAction(async () => {
                  const updated = await invoke<ManagedLink>("validate_link", { id: selected.id });
                  setLinks((current) => current.map((link) => (link.id === updated.id ? updated : link)));
                }, "검증했습니다.")}>
                  <RefreshCw size={16} />
                  검증
                </button>
                <button onClick={() => runAction(() => invoke("reveal_path", { path: selected.original_path }))}>
                  <FolderOpen size={16} />
                  링크 위치 열기
                </button>
                <button onClick={() => runAction(() => invoke("reveal_path", { path: selected.target_path }))}>
                  <FolderOpen size={16} />
                  대상 위치 열기
                </button>
                <button onClick={() => runAction(async () => {
                  const updated = await invoke<ManagedLink[]>("remove_from_manager", { id: selected.id });
                  setLinks(updated);
                  setSelectedId(updated[0]?.id);
                }, "관리 목록에서 제거했습니다.")}>
                  <XCircle size={16} />
                  목록에서 제거
                </button>
                <button className="danger" disabled={!isAdmin} onClick={() => runAction(async () => {
                  const updated = await invoke<ManagedLink[]>("delete_link", { id: selected.id });
                  setLinks(updated);
                  setSelectedId(updated[0]?.id);
                }, "링크를 삭제했습니다. 대상 폴더는 유지됩니다.")}>
                  <Trash2 size={16} />
                  링크 삭제
                </button>
              </div>
            </>
          ) : (
            <div className="empty">왼쪽 트리에서 링크를 선택하세요.</div>
          )}
        </section>
      </section>

      {replaceOpen ? (
        <ReplaceDialog
          onClose={() => setReplaceOpen(false)}
          onCreated={(link) => {
            setLinks((current) => [...current.filter((item) => item.id !== link.id), link]);
            setSelectedId(link.id);
            setReplaceOpen(false);
            setMessage("폴더를 심볼릭 링크로 대체했습니다.");
          }}
        />
      ) : null}

      {importOpen ? (
        <ImportDialog
          onClose={() => setImportOpen(false)}
          onImported={async () => {
            await refresh();
            setImportOpen(false);
            setMessage("기존 링크를 추가했습니다.");
          }}
        />
      ) : null}
    </main>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="field">
      <span>{label}</span>
      <code>{value}</code>
    </div>
  );
}

function ReplaceDialog({ onClose, onCreated }: { onClose: () => void; onCreated: (link: ManagedLink) => void }) {
  const [original, setOriginal] = React.useState("");
  const [root, setRoot] = React.useState("");
  const [preview, setPreview] = React.useState<ReplacePreview>();
  const [error, setError] = React.useState("");

  async function chooseOriginal() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") setOriginal(selected);
  }

  async function chooseRoot() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") setRoot(selected);
  }

  async function updatePreview() {
    if (!original || !root) return;
    try {
      setError("");
      setPreview(await invoke<ReplacePreview>("preview_replace_folder", { originalPath: original, storageRoot: root }));
    } catch (error) {
      setError(String(error));
      setPreview(undefined);
    }
  }

  React.useEffect(() => {
    updatePreview();
  }, [original, root]);

  const blocked =
    !preview ||
    !preview.is_admin ||
    !preview.original_exists ||
    preview.original_is_symlink ||
    preview.target_exists;

  return (
    <div className="modal-backdrop">
      <div className="modal">
        <h2>폴더 대체</h2>
        <PathInput label="원래 폴더" value={original} onChange={setOriginal} onPick={chooseOriginal} />
        <PathInput label="보관 루트" value={root} onChange={setRoot} onPick={chooseRoot} />
        {preview ? (
          <div className="preview">
            <Field label="계산된 이동 경로" value={preview.target_path} />
            {!preview.is_admin ? <Warning text="관리자 권한이 필요합니다." /> : null}
            {!preview.original_exists ? <Warning text="원래 폴더가 존재하지 않습니다." /> : null}
            {preview.original_is_symlink ? <Warning text="이미 심볼릭 링크입니다. 기존 링크 추가를 사용하세요." /> : null}
            {preview.target_exists ? <Warning text="대상 경로가 이미 존재합니다. 자동 병합/덮어쓰기는 하지 않습니다." /> : null}
          </div>
        ) : null}
        {error ? <Warning text={error} /> : null}
        <div className="modal-actions">
          <button onClick={onClose}>취소</button>
          <button
            className="primary"
            disabled={blocked}
            onClick={async () => onCreated(await invoke<ManagedLink>("replace_folder", { originalPath: original, storageRoot: root }))}
          >
            대체 실행
          </button>
        </div>
      </div>
    </div>
  );
}

function ImportDialog({ onClose, onImported }: { onClose: () => void; onImported: () => void }) {
  const [single, setSingle] = React.useState("");
  const [scanRoot, setScanRoot] = React.useState("");
  const [results, setResults] = React.useState<ScanResult[]>([]);
  const [selected, setSelected] = React.useState<Set<string>>(new Set());
  const [error, setError] = React.useState("");
  const [isScanning, setIsScanning] = React.useState(false);
  const [scanProgress, setScanProgress] = React.useState<ScanProgress>();
  const [scanLog, setScanLog] = React.useState<string[]>([]);
  const logRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    const unlisten = listen<ScanProgress>("scan-progress", (event) => {
      setScanProgress(event.payload);
      setScanLog((current) => {
        const next = [...current, event.payload.current_path];
        return next.slice(-160);
      });
      if (event.payload.done) {
        setIsScanning(false);
      }
    });

    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, []);

  React.useEffect(() => {
    const log = logRef.current;
    if (log) {
      log.scrollTop = log.scrollHeight;
    }
  }, [scanLog]);

  async function pickSingle() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") setSingle(selected);
  }

  async function pickScanRoot() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") setScanRoot(selected);
  }

  async function scan() {
    try {
      setError("");
      setResults([]);
      setSelected(new Set());
      setScanLog([]);
      setScanProgress(undefined);
      setIsScanning(true);
      const found = await invoke<ScanResult[]>("scan_existing_links", { rootPath: scanRoot });
      setResults(found);
      setSelected(new Set(found.filter((item) => !item.already_managed).map((item) => item.original_path)));
      setIsScanning(false);
    } catch (error) {
      setIsScanning(false);
      setError(String(error));
    }
  }

  async function importAll() {
    try {
      setError("");
      if (single) {
        await invoke("import_existing_link", { originalPath: single });
      }
      for (const path of selected) {
        await invoke("import_existing_link", { originalPath: path });
      }
      onImported();
    } catch (error) {
      setError(String(error));
    }
  }

  return (
    <div className="modal-backdrop">
      <div className="modal wide">
        <h2>기존 링크 추가</h2>
        <PathInput label="단일 링크" value={single} onChange={setSingle} onPick={pickSingle} />
        <div className="divider" />
        <PathInput label="스캔 루트" value={scanRoot} onChange={setScanRoot} onPick={pickScanRoot} />
        <button onClick={scan} disabled={!scanRoot || isScanning}>
          <Search size={16} />
          {isScanning ? "스캔 중" : "루트 스캔"}
        </button>
        <div className="scan-progress">
          <div className="scan-summary">
            <span>스캔한 폴더 {scanProgress?.scanned_count ?? 0}</span>
            <span>발견한 링크 {scanProgress?.found_count ?? results.length}</span>
          </div>
          <div className="scan-log" ref={logRef}>
            {scanLog.length ? (
              scanLog.map((line, index) => <code key={`${line}-${index}`}>{line}</code>)
            ) : (
              <span>스캔을 시작하면 현재 확인 중인 경로가 표시됩니다.</span>
            )}
          </div>
        </div>
        <div className="scan-list">
          {results.map((item) => (
            <label key={item.original_path} className="scan-row">
              <input
                type="checkbox"
                disabled={item.already_managed}
                checked={selected.has(item.original_path)}
                onChange={(event) => {
                  const next = new Set(selected);
                  if (event.currentTarget.checked) next.add(item.original_path);
                  else next.delete(item.original_path);
                  setSelected(next);
                }}
              />
              <span>{item.original_path}</span>
              <small>{item.already_managed ? "이미 등록됨" : item.target_path}</small>
            </label>
          ))}
        </div>
        {error ? <Warning text={error} /> : null}
        <div className="modal-actions">
          <button onClick={onClose}>취소</button>
          <button className="primary" onClick={importAll} disabled={!single && selected.size === 0}>
            추가
          </button>
        </div>
      </div>
    </div>
  );
}

function PathInput({
  label,
  value,
  onChange,
  onPick,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onPick: () => void;
}) {
  return (
    <label className="path-input">
      <span>{label}</span>
      <div>
        <input value={value} onChange={(event) => onChange(event.currentTarget.value)} placeholder="경로를 입력하거나 선택" />
        <button onClick={onPick} type="button">
          <FolderOpen size={16} />
        </button>
      </div>
    </label>
  );
}

function Warning({ text }: { text: string }) {
  return (
    <div className="warning">
      <AlertTriangle size={16} />
      {text}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
