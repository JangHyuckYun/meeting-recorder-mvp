# meeting-recorder-mvp

실시간 회의 녹음 + 화자분리 전사 + AI 회의록 생성 데스크톱 앱 MVP (macOS 우선).

- 실시간 녹음 → 화자분리 STT → 라이브 회의록 초안 → 부분 수정 → 회의별 히스토리 조회
- STT/화자분리/요약 모델은 자체 AI 서버(WhisperLive + LiteLLM, docker)에 우선 서빙, 상용 API(ElevenLabs Scribe)는 옵션
- v1 스코프: QR 페어링/로그인 제외, macOS 단일 기기 우선

## 구조 (front / api)

Tauri 앱이라 프론트와 백엔드가 한 빌드 단위로 묶여 있다. 물리적으로 `front/`, `api/` 디렉토리로
분리하면 `tauri.conf.json`의 `frontendDist: "../dist"`와 `beforeDevCommand: "pnpm dev"` 규약이
깨지므로, 디렉토리 이동 대신 아래처럼 역할을 명시한다.

| 역할 | 경로 | 스택 |
|------|------|------|
| **front** (프론트엔드) | `apps/desktop/src` | React 19 + TypeScript + Vite + Tailwind |
| **api** (백엔드) | `apps/desktop/src-tauri` | Rust + Tauri v2 (녹음/STT/요약/저장 커맨드) |
| **infra** (자체 AI 서버) | `infra/stt-server` | Python + Docker (WhisperLive STT, 화자분리 워커) |
| 기획/실행 계획 문서 | `.omo/plans` | Markdown |

`api` 내부:

- `src-tauri/src/audio` — 오디오 캡처 파이프라인
- `src-tauri/src/stt`, `src/asr` — STT / 화자분리 클라이언트 (자체 서버 + ElevenLabs)
- `src-tauri/src/minutes` — LLM 회의록 생성 (LiteLLM 게이트웨이)
- `src-tauri/src/storage` — SQLite 저장 + 마이그레이션

## 실행

```bash
cd apps/desktop
pnpm install

pnpm tauri dev   # 데스크톱 앱 (front + api 동시 기동)
pnpm dev         # 브라우저에서 프론트만
pnpm test        # 프론트 테스트 (vitest)

cd src-tauri && cargo test   # 백엔드 테스트
```

자체 STT 서버는 `infra/stt-server/README.md` 참고.

## 설정 / 시크릿

API 키(ElevenLabs 등)는 **저장소에 커밋하지 않는다.** 앱 실행 후 Settings 화면에서 입력하면
OS 키체인(SecretStore)에 저장된다. STT/LLM 서버 주소도 Settings에서 변경 가능하며,
소스의 `192.168.1.189:*` 값은 개발용 LAN 기본값일 뿐이다.
