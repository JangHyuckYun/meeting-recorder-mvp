# WhisperLive STT server

192.168.1.189의 **GPU1 (RTX 3070 8 GB)만** 사용하는 한국어 실시간 STT + 온라인 화자 클러스터링 서버다.

## 구성

| 항목 | 값 |
| --- | --- |
| WebSocket | `ws://192.168.1.189:9090` |
| OpenAI 호환 REST | `http://192.168.1.189:9091/v1/audio/transcriptions` |
| OpenAPI/헬스체크 | `http://192.168.1.189:9091/openapi.json` |
| 백엔드 | `faster_whisper` |
| 모델 | `mobiuslabsgmbh/faster-whisper-large-v3-turbo` (`large-v3-turbo`) |
| WhisperLive | 0.9.0, upstream commit `bde730b81bb23b5286470524d5f527caf6a8b9e0` |
| 화자 임베딩 | `pyannote/wespeaker-voxceleb-resnet34-LM` |
| GPU | host device `1` only (`NVIDIA_VISIBLE_DEVICES=1` + Compose `device_ids: ["1"]`) |

모델은 named volume `meeting-recorder-stt_stt-model-cache`에 캐시된다. 최초 WebSocket 연결에서는 Whisper 및 화자 임베딩 모델을 다운로드하므로 준비 시간이 추가로 걸린다. 공개 WeSpeaker 체크포인트의 기존 직렬화 형식을 PyTorch 2.6+에서 읽을 수 있도록 컨테이너에 `TORCH_FORCE_NO_WEIGHTS_ONLY_LOAD=1`을 한정 적용한다.

## 배포

```bash
ssh 192.168.1.189 'mkdir -p ~/whisperlive'
scp Dockerfile docker-compose.yml 192.168.1.189:~/whisperlive/
ssh 192.168.1.189 'cd ~/whisperlive && docker compose up -d --build'
```

기존 서비스를 건드리지 않고 이 Compose project만 조회/재배포한다.

```bash
ssh 192.168.1.189 'cd ~/whisperlive && docker compose ps'
ssh 192.168.1.189 'cd ~/whisperlive && docker compose logs -f whisperlive'
```

## 헬스체크

```bash
curl -fsS http://192.168.1.189:9091/openapi.json >/dev/null
ssh 192.168.1.189 \
  'docker inspect --format="{{.State.Status}} {{.State.Health.Status}}" meeting-recorder-whisperlive'
```

헬스체크는 API 프로세스의 응답 여부를 검사한다. 모델 추론까지 확인하려면 아래 WebSocket 전사 검증이 필요하다.

## 화자분리 옵션

WhisperLive의 온라인 speaker embedding clustering은 **WebSocket** 세션 옵션으로 활성화한다.

```json
{
  "uid": "<uuid>",
  "language": "ko",
  "task": "transcribe",
  "model": "large-v3-turbo",
  "use_vad": true,
  "enable_diarization": true,
  "max_speakers": 4,
  "diarization_threshold": 0.55
}
```

완료된 세그먼트에는 `"speaker": "SPEAKER_00"` 형식의 라벨이 포함된다. `max_speakers`는 클러스터 수 상한이고, `diarization_threshold`를 낮추면 화자를 더 적극적으로 병합한다(기본값 `0.55`). 화자 라벨은 세션별 상대 라벨이며 사람 이름이 아니다.

REST 엔드포인트는 일반 전사에 사용할 수 있지만, 익명 온라인 클러스터링 옵션을 받지 않는다. REST에서 화자 필드를 받으려면 `response_format=verbose_json`과 함께 `known_speaker_names` 및 동일 개수의 `known_speaker_references` 파일을 전달해야 한다.

## REST 전사 예시

```bash
curl -fsS http://192.168.1.189:9091/v1/audio/transcriptions \
  -F model=whisper-1 \
  -F language=ko \
  -F response_format=verbose_json \
  -F file=@sample.m4a
```
