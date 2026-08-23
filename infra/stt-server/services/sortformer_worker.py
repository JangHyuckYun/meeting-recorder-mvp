#!/usr/bin/env python3
"""NeMo Streaming Sortformer diarization worker.
Receives 16kHz mono PCM chunks, returns speaker-activity frame events.

Protocol: WebSocket binary (float32 PCM) → JSON speaker events
"""

import asyncio
import json
import sys
import struct

try:
    import numpy as np
except ImportError:
    np = None


class SortformerWorker:
    """Per-connection Sortformer diarization worker."""

    def __init__(self):
        self._model = None
        self._buffer: list[float] = []
        self._frame_count = 0

    def _load_model(self):
        """Lazy-load the Sortformer model."""
        if self._model is not None:
            return
        try:
            from nemo.collections.asr.models import EncDecDiarLabelModel
            self._model = EncDecDiarLabelModel.from_pretrained(
                "nvidia/diar_streaming_sortformer_4spk-v2.1"
            )
            self._model = self._model.to("cuda")
            self._model.eval()
            print("Sortformer model loaded on CUDA", file=sys.stderr)
        except Exception as e:
            print(f"WARNING: Could not load Sortformer: {e}", file=sys.stderr)
            self._model = None

    def accept_waveform(self, samples: list[float], sample_rate: int = 16000):
        """Accept 16kHz mono PCM and buffer for diarization."""
        self._buffer.extend(samples)
        self._frame_count += len(samples)

    def get_speaker_activity(self) -> list[dict]:
        """Run diarization on the buffered audio. Returns speaker activity frames."""
        self._load_model()
        if self._model is None or len(self._buffer) < 16000:  # Need at least 1s
            return []

        if np is None:
            return []

        arr = np.array(self._buffer[:16000 * 30], dtype=np.float32)  # max 30s window
        self._buffer = []

        try:
            # Sortformer expects shape (1, T) or (T,)
            import torch
            with torch.no_grad():
                logits = self._model(arr[None, :])
            # Parse speaker activity from logits
            activities = []
            for frame_idx in range(0, len(arr), 160):  # 10ms frames at 16kHz
                frame_start = frame_idx / 16000.0
                activities.append({
                    "frame_offset": frame_idx,
                    "time_seconds": round(frame_start, 3),
                    "active_speakers": 0,  # Simplified; real parsing needs model output format
                })
            return activities
        except Exception as e:
            print(f"Sortformer inference error: {e}", file=sys.stderr)
            return []


# ---------------------------------------------------------------------------
# WebSocket server
# ---------------------------------------------------------------------------
try:
    from aiohttp import web
except ImportError:
    web = None


async def handle_diarize(request):
    ws = web.WebSocketResponse()
    await ws.prepare(request)
    worker = SortformerWorker()

    try:
        async for msg in ws:
            if msg.type == web.WSMsgType.BINARY:
                raw = msg.data
                count = len(raw) // 4
                samples = list(struct.unpack(f"{count}f", raw))
                worker.accept_waveform(samples)

                activities = worker.get_speaker_activity()
                if activities:
                    await ws.send_json({
                        "type": "speaker_activity",
                        "activities": activities,
                        "frame_count": worker._frame_count,
                    })

            elif msg.type == web.WSMsgType.TEXT:
                data = json.loads(msg.data)
                if data.get("command") == "done":
                    break

            elif msg.type == web.WSMsgType.ERROR:
                break
    finally:
        pass

    return ws


async def handle_health(request):
    return web.json_response({"status": "ok", "service": "sortformer"})


def main():
    app = web.Application()
    app.router.add_get("/diarize", handle_diarize)
    app.router.add_get("/health", handle_health)

    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9094
    print(f"Starting Sortformer diarization worker on port {port}", file=sys.stderr)
    web.run_app(app, host="0.0.0.0", port=port)


if __name__ == "__main__":
    main()