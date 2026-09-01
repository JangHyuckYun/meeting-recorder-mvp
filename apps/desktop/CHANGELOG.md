# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Fixed

- 설정에서 ElevenLabs가 기본값/선택값인데 자체 모델 서버로 표시·저장되던 문제 (Rust 기본값 `self_hosted` → `elevenlabs`). 기존 DB의 `self_hosted` 시드값도 마이그레이션 0010으로 `elevenlabs`로 전환

### Changed

- ElevenLabs 선택 시 서버 URL 숨김
- 설정의 화자 수 항목 제거 (가져오기에서 설정)
- 공급자 유형 OpenAI / Anthropic / vLLM(OpenAI 호환) 분리

### Added

- OpenAI·Anthropic 공급자 인증 방식(API key / OAuth) 선택
- 마이그레이션 0009 `providers.auth_mode`
- 인앱 OAuth 로그인(PKCE, 브라우저 열기, localhost 콜백, 코드 붙여넣기 폴백) `start_oauth_login` / `complete_oauth_login`
