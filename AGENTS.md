# meeting-recorder-mvp — 작업 규칙

## 작업관리 (Notion, 단일 출처)
- DB: **meeting-recorder 작업관리** — https://app.notion.com/p/c20d684f5dc04466a4d9f4ab771def7e
  (data source `4ce977d5-24e1-4e03-8c28-8eb3ab6744c6`)
- 사용자 작업 요청 1건 = row 1개. 속성: 분류(신규/개선/버그/조사), 영역, 상태, 브랜치, PR, 요청일/완료일, 요약.
- 상태 lifecycle: `요청` → `진행중`(브랜치 생성 시) → `리뷰(PR)`(PR 오픈 시, PR URL 기입) → `완료`(머지 시, 완료일 기입). 막히면 `보류` + 사유.
- 작업내역은 row 페이지 본문 `## 작업내역`에 `- YYYY-MM-DD …` 로 누적. 에이전트 자체 todo는 휘발성, 여기 기록만 durable.

## 사고 기록
- `error-log.md`(레포 루트)에 재발 방지 규칙이 일자별로 있다. 작업 시작 전에 읽고, 새 사고는 같은 형식으로 추가한다.

## 개발 흐름
- 신규/개선 요청은 `main`(또는 선행 PR 브랜치)에서 `feat/<영역-요약>` 브랜치를 파고, 영역별로 PR을 나눈다.
- TDD: vitest(프론트) / cargo test(Rust) 테스트를 먼저 추가하고 구현. PR 전 `pnpm exec tsc --noEmit`, `pnpm vitest run`, `cargo check && cargo test` 통과.
- `apps/desktop/CHANGELOG.md` (Keep a Changelog) `## [Unreleased]`에 항목 추가.
- 커밋/PR 본문에 Notion row 링크를 남기고, PR URL을 row에 기입.

## 시크릿
- `apps/mcp/.env`(ELEVENLABS_API_KEY 등)와 각종 키는 읽거나 출력하지 않는다. 값이 필요하면 env 이름만 참조.
