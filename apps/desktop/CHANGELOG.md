## [Unreleased]

### Fixed

- ElevenLabs 전사가 용어집의 잘못된 항목(자모만 있는 값 등) 때문에 400 `invalid_keyword`로 전부 실패하던 문제 — 전송 전 용어 정규화, 실패 시 용어 없이 1회 재시도, 단어장 저장 시 검증
