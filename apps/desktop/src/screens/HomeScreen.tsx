/** S1 — Home. Notes list: search, folder filter chips, create folder, assign/delete
 * a recording's folder, open a note. Contract: HomeScreen({ onOpenNote }). */
import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui";
import { appClient, type Folder } from "@/platform/appClient";
import { errorMessage, formatDate, formatDuration, STATUS_LABELS } from "../formatters";
import type { Recording } from "../types";

interface HomeScreenProps {
  /** Called with a recording's id when its row is opened (navigates to S4). */
  onOpenNote: (recordingId: string) => void;
}

function EmptyHome() {
  return (
    <div className="history-empty">
      <h2>아직 저장된 회의가 없습니다</h2>
      <p>실시간 탭에서 첫 녹음을 시작하거나, 가져오기 탭에서 기존 녹음 파일을 가져오세요.</p>
    </div>
  );
}

export function HomeScreen({ onOpenNote }: HomeScreenProps) {
  const [recordings, setRecordings] = useState<Recording[]>([]);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [activeFolder, setActiveFolder] = useState<string | null>(null);
  const [showNewFolder, setShowNewFolder] = useState(false);
  const [newFolderName, setNewFolderName] = useState("");

  const load = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const [recordingList, folderList] = await Promise.all([
        appClient.listRecordings(),
        appClient.listFolders(),
      ]);
      setRecordings(recordingList);
      setFolders(folderList);
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const visible = useMemo(() => {
    const query = search.trim().toLowerCase();
    return recordings.filter((recording) => {
      if (activeFolder !== null && recording.folder_id !== activeFolder) return false;
      if (query && !recording.title.toLowerCase().includes(query)) return false;
      return true;
    });
  }, [recordings, search, activeFolder]);

  const createFolder = async () => {
    const name = newFolderName.trim();
    if (!name) return;
    try {
      const folder = await appClient.createFolder(name);
      setFolders((current) => [...current, folder]);
      setNewFolderName("");
      setShowNewFolder(false);
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    }
  };

  const assignFolder = async (recordingId: string, folderId: string | null) => {
    try {
      await appClient.assignRecordingFolder(recordingId, folderId);
      setRecordings((current) =>
        current.map((r) => (r.id === recordingId ? { ...r, folder_id: folderId } : r)),
      );
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    }
  };

  const removeRecording = async (recordingId: string) => {
    if (!window.confirm("이 녹음을 삭제할까요? 되돌릴 수 없습니다.")) return;
    try {
      await appClient.deleteRecording(recordingId);
      setRecordings((current) => current.filter((r) => r.id !== recordingId));
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    }
  };

  return (
    <section className="history-screen">
      <header className="history-topbar">
        <h1>노트</h1>
        <span className="history-count" data-numeric>
          {recordings.length}건
        </span>
        <Button variant="outline" size="sm" onClick={load}>
          새로고침
        </Button>
      </header>

      <div className="home-filters">
        <input
          className="home-search"
          type="search"
          placeholder="노트 검색 (제목)"
          value={search}
          onChange={(event) => setSearch(event.currentTarget.value)}
        />
        <div className="home-chips">
          <button
            type="button"
            className="home-chip"
            data-active={activeFolder === null}
            onClick={() => setActiveFolder(null)}
          >
            전체
          </button>
          {folders.map((folder) => (
            <button
              key={folder.id}
              type="button"
              className="home-chip"
              data-active={activeFolder === folder.id}
              onClick={() => setActiveFolder(folder.id)}
            >
              {folder.name}
            </button>
          ))}
          {showNewFolder ? (
            <form
              className="home-new-folder"
              onSubmit={(event) => {
                event.preventDefault();
                void createFolder();
              }}
            >
              <input
                autoFocus
                className="home-new-folder-input"
                placeholder="폴더 이름"
                value={newFolderName}
                onChange={(event) => setNewFolderName(event.currentTarget.value)}
                onBlur={() => {
                  if (!newFolderName.trim()) setShowNewFolder(false);
                }}
              />
            </form>
          ) : (
            <button type="button" className="home-chip home-chip-add" onClick={() => setShowNewFolder(true)}>
              + 폴더
            </button>
          )}
        </div>
      </div>

      <div className="history-list ds-scroll">
        {error && (
          <div className="error-banner" role="alert">
            요청을 처리하지 못했습니다: {error}
          </div>
        )}

        {isLoading ? (
          <p className="history-loading">
            <span aria-hidden="true" />
            녹음 목록을 불러오는 중...
          </p>
        ) : visible.length === 0 ? (
          error ? null : <EmptyHome />
        ) : (
          <>
            <div className="history-row history-row-head home-row" aria-hidden="true">
              <span>제목</span>
              <span>상태</span>
              <span>기록 시각</span>
              <span className="history-cell-duration">길이</span>
              <span />
            </div>
            {visible.map((recording) => (
              <div
                className="history-row home-row"
                key={recording.id}
                role="button"
                tabIndex={0}
                onClick={() => onOpenNote(recording.id)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") onOpenNote(recording.id);
                }}
              >
                <span className="history-cell-title">{recording.title}</span>
                <span className="history-status" data-status={recording.status}>
                  {STATUS_LABELS[recording.status]}
                </span>
                <span className="history-cell-date" data-numeric>
                  {formatDate(recording.created_at)}
                </span>
                <span className="history-cell-duration" data-numeric>
                  {formatDuration(recording.duration_ms)}
                </span>
                <span className="home-row-actions" onClick={(event) => event.stopPropagation()}>
                  <select
                    aria-label="폴더 지정"
                    className="home-folder-select"
                    value={recording.folder_id ?? ""}
                    onChange={(event) => void assignFolder(recording.id, event.currentTarget.value || null)}
                  >
                    <option value="">미지정</option>
                    {folders.map((folder) => (
                      <option key={folder.id} value={folder.id}>
                        {folder.name}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    className="home-delete-btn"
                    aria-label="녹음 삭제"
                    onClick={() => void removeRecording(recording.id)}
                  >
                    삭제
                  </button>
                </span>
              </div>
            ))}
          </>
        )}
      </div>
    </section>
  );
}
