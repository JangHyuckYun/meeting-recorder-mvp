# meeting-recorder-mvp

실시간 회의 녹음 + 화자분리 전사 + AI 회의록 생성 데스크톱 앱 MVP (macOS 우선).

- 실시간 녹음 → 화자분리 STT → 라이브 회의록 초안 → 부분 수정 → 회의별 히스토리 조회
- STT/화자분리/요약 모델은 자체 AI 서버(vLLM, docker)에 우선 서빙, 상용 API는 옵션
- v1 스코프: QR 페어링/로그인 제외, macOS 단일 기기 우선

## 상태
스캐폴딩 단계. `.omo/plans/`에 실행 계획 문서 예정.
