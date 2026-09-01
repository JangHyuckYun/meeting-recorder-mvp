# error-log.md — 반복 금지 사고 기록

에이전트/오케스트레이션 운영 중 발생한 사고와 재발 방지 규칙. 새 세션은 작업 전에 이 파일을 읽는다.
형식: 날짜 → 사고 → 원인 → 재발 방지 규칙(명령형).

## 2026-09-01

### 1. codex `--background` 잡을 forwarder 종료로 죽임 (2회)
- **증상**: 10분 넘게 산출물 없음. `codex-companion status` → `No job found`.
- **원인**: `codex:codex-rescue` forwarder 에이전트를 "정리"한다고 `TaskStop` → 자식 codex 프로세스와 job 상태 파일까지 삭제됨.
- **규칙**: codex-rescue는 `--wait`로 띄운다. 결과가 오기 전엔 forwarder를 절대 `TaskStop` 하지 않는다. `--background`를 썼다면 job JSON(`~/.claude/plugins/data/codex-openai-codex/state/<repo>/jobs/`)이 존재하는지 먼저 확인하고 그 파일로만 폴링한다.

### 2. teammate가 trust/MCP 허용 프롬프트에 걸려 40분 무산출
- **증상**: sonnet verify 에이전트 3개가 20~40분 아무 산출물 없음.
- **원인**: 새 프로젝트 디렉터리/신규 `.mcp.json` 때문에 "Do you trust this project?" / MCP 허용 프롬프트에서 대기.
- **규칙**: spawn 전 대상 디렉터리 신뢰 여부·미허용 MCP 확인. 5~10분 내 산출물 0이면 프롬프트 대기로 간주하고 즉시 사용자에게 알린다. 끝난 에이전트는 `shutdown_request`가 아니라 `TaskStop`으로 확실히 종료한다.

### 3. codex가 cargo를 `/private/tmp` 새 target dir에 3개 동시 풀빌드 → 렉 + Gatekeeper "검사 중" 팝업 깜빡임
- **증상**: 시스템 전체 렉, 화면에 `build-…` 검사 중 팝업이 계속 깜빡임(XprotectService CPU 45%).
- **원인**: 프롬프트로 지정한 `CARGO_TARGET_DIR`(기존 캐시)을 codex가 무시하고 `/private/tmp/meeting-recorder-cargo-target{,2,3}`에 의존성 전체를 3개 프로세스 병렬 빌드. 새로 컴파일된 `build-script-build` 실행파일마다 macOS Gatekeeper가 검사 팝업을 띄움.
- **규칙**:
  - Rust 빌드가 필요한 codex 작업은 **동시에 1개만** 실행한다(worktree 병렬은 프론트 전용 작업에만).
  - 프롬프트에 명시: "cargo는 `CARGO_TARGET_DIR=<메인 레포>/apps/desktop/src-tauri/target` 로만, 한 번에 하나만 실행. `/tmp`·새 target dir 생성 금지. 위반 시 중단하고 보고."
  - 오케스트레이터는 codex 시작 3~5분 후 `pgrep -fl 'bin/cargo'`로 cargo 개수와 cwd/target을 확인하고, 2개 이상이거나 `/tmp` 경로면 즉시 죽인다.
  - 렉/깜빡임 신고가 오면 가장 먼저 `ps -Ao pcpu,comm -r | head`와 cargo/rustc 프로세스를 본다.

### 4. codex "pre-existing failure" 보고를 그대로 믿을 뻔함
- **증상**: cargo test 1개 실패를 codex가 "기존 실패"라고 보고. 실제로는 우리 지시("첫 text 블록만")로 생긴 회귀.
- **규칙**: 에이전트가 "기존 실패"라 하면 `git stash`로 변경 전 상태에서 같은 테스트를 돌려 직접 확인한다.

### 5. 마이그레이션 파일을 개발 중 수정해 dev DB `VersionMismatch`
- **증상**: 앱 기동 시 `Migrate(VersionMismatch(8))` 패닉.
- **원인**: 0008 마이그레이션이 이미 dev DB에 적용된 뒤 같은 파일을 두 번 더 수정(체크섬 불일치).
- **규칙**: 적용된 마이그레이션 파일은 수정하지 않고 새 번호를 만든다. 개발 중 부득이하면 dev DB 백업 후 해당 버전 행·컬럼을 되돌리고 재적용한다.

### 6. 에이전트가 `tee -` 실수로 레포에 `-` 파일 생성
- **규칙**: PR 전 `git status --short`에서 이름이 이상한 untracked 파일(`-`, `nohup.out` 등)을 확인하고 정리한다.

### 7. 죽은 codex 잡을 "running"으로 믿고 38분 대기
- **증상**: job JSON은 `running/editing`인데 로그는 08:05에서 멈춤, 해당 worktree cwd의 codex 프로세스 없음. 작업물은 이미 완성 상태였음.
- **원인**: 사고 3 정리 중 `pkill`로 함께 종료된 것으로 추정. job 상태 파일은 프로세스가 죽어도 갱신되지 않는다.
- **규칙**: codex 대기는 job JSON만 보지 말고 **로그 마지막 타임스탬프 + `pgrep`/`lsof` cwd**를 같이 본다. 로그가 5분 이상 멈추면 죽은 것으로 간주하고 worktree 상태를 직접 검증(tsc/vitest/cargo)해 이어간다. 끝난 codex 프로세스(예: note)는 잡 완료 확인 후 `pkill -f <worktree경로>`로 정리한다.

### 8. codex가 이미 적용된 마이그레이션(0004)을 수정
- **증상**: 설정 브랜치 diff에 `0004_stt_engine.sql` 변경이 포함(시드값 self_hosted→elevenlabs).
- **영향**: 머지됐다면 모든 기존 DB가 `VersionMismatch(4)`로 기동 불가.
- **규칙**: codex 프롬프트에 "기존 마이그레이션 파일 수정 금지, 새 번호로만"을 항상 넣고, PR 전 `git status`에서 `migrations/` 아래 `M`(수정) 항목이 있으면 무조건 되돌린다. (규칙 5의 강화판)

### 9. codex 샌드박스는 worktree 밖(공유 cargo target)에 쓸 수 없다
- **증상**: `cargo check`가 `.cargo-build-lock` 생성 실패로 중단되거나(정직한 경우), 규칙을 어기고 `/tmp`에 새 target을 만들어 풀빌드(사고 3).
- **규칙**: worktree에서 도는 codex에는 **cargo를 아예 실행시키지 않는다**("Do NOT run cargo"). Rust 검증은 오케스트레이터가 공유 `CARGO_TARGET_DIR`로 직접 돌린다. 프론트(tsc/vitest)만 codex에 맡긴다.

### 10. 스택 PR 3개를 각각만 검증하고 통합 검증을 안 함
- **증상**: 통합 브랜치에서 AppState 필드 충돌(#3/#4)과 기존 테스트의 옛 기본값 단정(`SelfHosted`)이 드러남. 개별 PR은 모두 초록이었다.
- **규칙**: 같은 파일(AppState, invoke_handler, CHANGELOG)을 건드리는 PR이 2개 이상이면 머지 전에 로컬 통합 브랜치를 만들어 tsc/vitest/cargo test를 한 번 더 돌린다. 기본값을 바꾸는 변경은 `rg`로 옛 값을 단정하는 테스트를 먼저 찾는다.

### 11. 사용자 데이터(용어집)의 테스트 값이 실서비스 요청으로 그대로 전송돼 전사 전체 실패
- **증상**: ElevenLabs 400 `invalid_keyword` (용어집 `ㄴㅇㅁ…`).
- **규칙**: 외부 API로 나가는 사용자 입력은 전송 직전에 정규화하고, 선택적 파라미터(keyterms 등) 때문에 핵심 기능(전사)이 통째로 실패하지 않게 폴백을 둔다. 저장 시점(trust boundary)에서도 검증한다. — PR #5
