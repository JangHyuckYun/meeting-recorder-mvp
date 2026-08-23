#!/usr/bin/env python3
"""faster-whisper Turbo correction worker.
Receives stable chunk windows, re-decodes with larger model, emits COMMITTED captions.

Protocol: WebSocket JSON at /correct
  Input:  {"segments": [...], "audio_base64": "..."}
  Output: {"segment_id": ..., "text": ..., "status": "committed"}
"""
import asyncio
import base64
import json
import struct
import sys
import uuid
from io import BytesIO

try:
    import numpy as np
except ImportError:
    np = None


class TurboWorker:
    """faster-whisper large-v3-turbo correction worker."""

    def __init__(self, model_size: str = "large-v3-turbo", device: str = "cuda"):
        self._model = None
        self._model_size = model_size
        self._device = device

    def _load_model(self):
        if self._model is not None:
            return
        try:
            from faster_whisper import WhisperModel
            self._model = WhisperModel(
                self._model_size,
                device=self._device,
                compute_type="float16",
            )
            print(f"Turbo model loaded on {self._device}", file=sys.stderr)
        except Exception as e:
            print(f"WARNING: Could not load Turbo model: {e}", file=sys.stderr)
            self._model = None

    def transcribe(self, audio_16k: list[float], language: str = "ko") -> list[dict]:
        """Transcribe 16kHz mono audio. Returns list of segment dicts."""
        self._load_model()
        if self._model is None or np is None:
            return [{"text": "[turbo unavailable]", "status": "committed",
                      "segment_id": str(uuid.uuid4())}]

        arr = np.array(audio_16k, dtype=np.float32)
        segments = []
        try:
            raw_segments, info = self._model.transcribe(arr, language=language,
                                                        beam_size=5, word_timestamps=False)
            for i, seg in enumerate(raw_segments):
                segments.append({
                    "segment_id": str(uuid.uuid4()),
                    "text": seg.text.strip(),
                    "start_sample": int(seg.start * 16000),
                    "end_sample": int(seg.end * 16000),
                    "status": "committed",
                    "speaker_label": None,
                })
        except Exception as e:
            print(f"Turbo transcription error: {e}", file=sys.stderr)
            segments.append({"text": "[turbo error]", "status": "committed",
                              "segment_id": str(uuid.uuid4())})

        return segments if segments else [{"text": "", "status": "committed",
                                            "segment_id": str(uuid.uuid4())}]


# ---------------------------------------------------------------------------
# WebSocket server
# ---------------------------------------------------------------------------
try:
    from aiohttp import web
except ImportError:
    web = None


async def handle_correct(request):
    ws = web.WebSocketResponse()
    await ws.prepare(request)
    worker = TurboWorker()

    try:
        async for msg in ws:
            if msg.type == web.WSMsgType.TEXT:
                data = json.loads(msg.data)
                command = data.get("command", "")

                if command == "transcribe":
                    audio_b64 = data.get("audio_base64", "")
                    audio_bytes = base64.b64decode(audio_b64)
                    count = len(audio_bytes) // 4
                    samples = list(struct.unpack(f"{count}f", audio_bytes))
                    segments = worker.transcribe(samples, language=data.get("language", "ko"))
                    await ws.send_json({"type": "result", "segments": segments})

                elif command == "done":
                    break

            elif msg.type == web.WSMsgType.ERROR:
                break
    finally:
        pass

    return ws


async def handle_health(request):
    return web.json_response({"status": "ok", "service": "turbo"})


def main():
    app = web.Application()
    app.router.add_get("/correct", handle_correct)
    app.router.add_get("/health", handle_health)

    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9095
    print(f"Starting Turbo correction worker on port {port}", file=sys.stderr)
    web.run_app(app, host="0.0.0.0", port=port)


if __name__ == "__main__":
    main()