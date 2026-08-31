/** S10 — Share.
 * ponytail: no share-link backend exists yet (no appClient.create/getShareLink).
 * Everything here is local UI state for the mockup — no link is ever generated
 * or persisted. Wire this up to real endpoints once the backend adds them.
 */
import { useState } from "react";
import { Button } from "@/components/ui";
import "../styles/share.css";

interface ShareScreenProps {
  recordingId: string;
}

type Permission = "view" | "comment" | "edit";

const PERMISSIONS: { value: Permission; label: string }[] = [
  { value: "view", label: "보기" },
  { value: "comment", label: "댓글" },
  { value: "edit", label: "편집" },
];

interface Invitee {
  name: string;
  email: string;
  permission: Permission;
}

export function ShareScreen({ recordingId }: ShareScreenProps) {
  const [linkPermission, setLinkPermission] = useState<Permission>("view");
  const [invitees, setInvitees] = useState<Invitee[]>([]);
  const [inviteInput, setInviteInput] = useState("");
  const [teamFolderAuto, setTeamFolderAuto] = useState(false);
  const [passwordProtected, setPasswordProtected] = useState(false);

  const handleInvite = () => {
    const value = inviteInput.trim();
    if (!value) return;
    setInvitees((prev) => [...prev, { name: value, email: value, permission: "view" }]);
    setInviteInput("");
  };

  return (
    <section className="share-screen">
      <header className="share-header">
        <p className="share-eyebrow">SHARE</p>
        <h1>공유</h1>
        <p className="share-subtitle" data-numeric>
          노트 {recordingId}
        </p>
      </header>

      <div className="share-body">
        <div className="share-notice" role="status">
          로컬 미리보기 — 공유 서버 준비 중입니다. 여기서의 변경 사항은 저장되거나 실제로 공유되지 않습니다.
        </div>

        <div className="share-card">
          <p className="share-card-label">공유 링크</p>
          <div className="share-link-row">
            <input
              className="share-link-input"
              value=""
              readOnly
              disabled
              placeholder="공유 서버 연동 전에는 링크가 생성되지 않습니다"
              aria-label="공유 링크"
              data-numeric
            />
            <Button variant="outline" size="sm" disabled title="공유 서버 준비 중">
              복사
            </Button>
          </div>
          <div className="share-permission-row">
            <span>링크 소지자 권한</span>
            {PERMISSIONS.map((p) => (
              <button
                key={p.value}
                type="button"
                className="share-permission-btn"
                aria-pressed={linkPermission === p.value}
                onClick={() => setLinkPermission(p.value)}
              >
                {p.label}
              </button>
            ))}
          </div>
        </div>

        <div className="share-card">
          <p className="share-card-label">초대된 사람 ({invitees.length})</p>
          {invitees.map((invitee, i) => (
            <div key={i} className="share-invitee-row">
              <span className="share-invitee-avatar" aria-hidden="true">
                {invitee.name.slice(0, 1)}
              </span>
              <div className="share-invitee-main">
                <div className="share-invitee-name">{invitee.name}</div>
                <div className="share-invitee-email">{invitee.email}</div>
              </div>
              <span className="share-permission-btn" aria-pressed="true">
                {PERMISSIONS.find((p) => p.value === invitee.permission)?.label}
              </span>
            </div>
          ))}
          <div className="share-invite-row">
            <input
              className="share-invite-input"
              value={inviteInput}
              onChange={(e) => setInviteInput(e.target.value)}
              placeholder="이름 또는 이메일로 초대"
              onKeyDown={(e) => {
                if (e.key === "Enter") handleInvite();
              }}
            />
            <Button size="sm" onClick={handleInvite} disabled={!inviteInput.trim()}>
              초대
            </Button>
          </div>
        </div>

        <div className="share-card">
          <div className="share-toggle-row">
            <span>팀 폴더 자동 공유</span>
            <button
              type="button"
              className="share-permission-btn"
              aria-pressed={teamFolderAuto}
              onClick={() => setTeamFolderAuto((v) => !v)}
            >
              {teamFolderAuto ? "ON" : "OFF"}
            </button>
          </div>
          <div className="share-toggle-row">
            <span>비밀번호 보호</span>
            <button
              type="button"
              className="share-permission-btn"
              aria-pressed={passwordProtected}
              onClick={() => setPasswordProtected((v) => !v)}
            >
              {passwordProtected ? "ON" : "OFF"}
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}
