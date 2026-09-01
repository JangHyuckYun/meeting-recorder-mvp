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
