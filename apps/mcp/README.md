# Meeting Transcribe MCP

데스크톱 앱 없이 오디오 파일을 화자 분리 전사합니다. Python 3.11+, `uv`, `ffmpeg`(없으면 macOS `afconvert`)가 필요합니다.

```bash
uv sync
uv run meeting-mcp
uv run python test_smoke.py
```

Codex 설정(`/Users/janghyuck/Desktop/personal_github/ai_workspace/.mcp.json`):

```json
{"mcpServers":{"meeting-transcribe":{"command":"uv","args":["run","--directory","/Users/janghyuck/Desktop/personal_github/ai_workspace/meeting-recorder-mvp/apps/mcp","meeting-mcp"]}}}
```

- 설정 파일: `~/.config/meeting-mcp/config.json` (자동 생성, `0600`)
- 환경 변수: `ELEVENLABS_API_KEY`, `MEETING_MCP_LOCAL_WS_URL`
- 선택적 `.env`: 이 디렉터리의 `.env` (`KEY=VALUE`)
- `list_providers`: 기본 provider와 각 provider 설정/준비 상태를 조회합니다.
- `configure_provider(provider, settings)`: provider 설정을 저장합니다. `speakers`는 `"auto"` 또는 양의 정수이며, local의 `"auto"`는 WhisperLive handshake가 숫자를 요구하므로 기존 기본값 `4`를 사용합니다.
- `transcribe(file_path, provider?, language?, speakers?, output_dir?, formats?)`: `speakers`를 생략하면 provider 설정, `"auto"`면 자동, 양의 정수면 고정 화자 수를 사용합니다.

Google provider 추가: provider 함수 하나, `PROVIDERS` dict 항목 하나, `DEFAULTS` 항목 하나를 추가합니다.
