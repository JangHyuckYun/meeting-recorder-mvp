#!/usr/bin/env python3
"""ASR v2 Gateway — the main routing service.
Accepts client WebSocket connections, fans out to Zipformer+Sortformer,
routes stable chunks to Turbo, and emits CaptionEvent JSON.

Protocol: WebSocket at /ws
  Client → Gateway: binary float32 PCM (16kHz mono)
  Gateway → Client: JSON CaptionEvent
"""

import asyncio
import json
import struct
import sys
import uuid
from datetime import datetime, timezone

try:
    import aiohttp
    from aiohttp import web
except ImportError:
    aiohttp = None
    web = None


class CaptionState:
    """Tracks caption state for one client session."""

    def __init__(self):
        self.segments: dict[str, dict] = {}
        self.sequence = 0

    def apply_partial(self, segment_id: str, text: str, start_sample: int, end_sample: int,
                      speaker_label: str | None = None) -> dict:
        event = {
            "segment_id": segment_id,
            "start_sample": start_sample,
            "end_sample": end_sample,
            "text": text,
            "status": "partial",
            "speaker_label": speaker_label,
            "overlap": None,
            "supersedes": [],
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        self.segments[segment_id] = event
        self.sequence += 1
        return event

    def apply_stable(self, segment_id: str, text: str, start_sample: int, end_sample: int,
                     speaker_label: str | None = None) -> dict:
        event = {
            "segment_id": segment_id,
            "start_sample": start_sample,
            "end_sample": end_sample,
            "text": text,
            "status": "stable",
            "speaker_label": speaker_label,
            "overlap": None,
            "supersedes": [],
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        self.segments[segment_id] = event
        self.sequence += 1
        return event

    def apply_committed(self, segment_id: str, text: str, start_sample: int, end_sample: int,
                        speaker_label: str | None = None) -> dict:
        event = {
            "segment_id": segment_id,
            "start_sample": start_sample,
            "end_sample": end_sample,
            "text": text,
            "status": "committed",
            "speaker_label": speaker_label,
            "overlap": None,
            "supersedes": [],
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        self.segments[segment_id] = event
        self.sequence += 1
        return event

    def apply_revised(self, segment_id: str, text: str, start_sample: int, end_sample: int,
                      supersedes: list[str], speaker_label: str | None = None) -> dict:
        event = {
            "segment_id": segment_id,
            "start_sample": start_sample,
            "end_sample": end_sample,
            "text": text,
            "status": "revised",
            "speaker_label": speaker_label,
            "overlap": None,
            "supersedes": supersedes,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        self.segments[segment_id] = event
        self.sequence += 1
        # Remove superseded segments
        for sid in supersedes:
            self.segments.pop(sid, None)
        return event


class GatewaySession:
    """Manages one client session through the ASR pipeline."""

    def __init__(self, ws, session_id: str):
        self.ws = ws
        self.session_id = session_id
        self.caption_state = CaptionState()
        self._buffer: list[float] = []
        self._zipformer_url = "http://zipformer:9093/asr"
        self._sortformer_url = "http://sortformer:9094/diarize"
        self._turbo_url = "http://turbo:9095/correct"

    async def handle_audio(self, samples: list[float]):
        self._buffer.extend(samples)
        # In a real deployment, fan out to internal workers here.
        # For now, emit a heartbeat/progress caption.
        if len(self._buffer) >= 5120:  # 320ms chunks
            chunk_count = len(self._buffer) // 5120
            for i in range(chunk_count):
                _ = self._buffer[:5120]
                self._buffer = self._buffer[5120:]
                sid = str(uuid.uuid4())
                event = self.caption_state.apply_partial(
                    sid, "…", (i * 5120), (i + 1) * 5120
                )
                await self.ws.send_json(event)

    async def handle_done(self):
        """Flush remaining captions and emit committed events."""
        if self._buffer:
            sid = str(uuid.uuid4())
            final_text = f"[{len(self._buffer)} samples buffered]"
            event = self.caption_state.apply_committed(
                sid, final_text, 0, len(self._buffer)
            )
            await self.ws.send_json(event)
        self._buffer = []


# ---------------------------------------------------------------------------
# WebSocket server
# ---------------------------------------------------------------------------

async def handle_ws(request):
    ws = web.WebSocketResponse()
    await ws.prepare(request)
    session_id = str(uuid.uuid4())
    session = GatewaySession(ws, session_id)

    try:
        async for msg in ws:
            if msg.type == web.WSMsgType.BINARY:
                raw = msg.data
                count = len(raw) // 4
                samples = list(struct.unpack(f"{count}f", raw))
                await session.handle_audio(samples)

            elif msg.type == web.WSMsgType.TEXT:
                data = json.loads(msg.data)
                if data.get("command") == "done":
                    await session.handle_done()
                    break
                elif data.get("command") == "snapshot":
                    await ws.send_json({
                        "type": "snapshot",
                        "segments": list(session.caption_state.segments.values()),
                        "sequence": session.caption_state.sequence,
                    })

            elif msg.type == web.WSMsgType.ERROR:
                break
    finally:
        pass

    return ws


async def handle_health(request):
    return web.json_response({
        "status": "ok",
        "service": "asr-gateway",
        "version": "2.0",
    })


async def handle_transcribe(request):
    """REST endpoint for offline file transcription."""
    reader = await request.multipart()
    field = await reader.next()
    if field is None:
        return web.json_response({"error": "no audio file"}, status=400)

    chunks = []
    while True:
        chunk = await field.read_chunk()
        if not chunk:
            break
        chunks.append(chunk)
    audio_data = b"".join(chunks)

    # Basic audio processing (placeholder for real Zipformer integration)
    return web.json_response({
        "status": "ok",
        "segments": [
            {
                "segment_id": str(uuid.uuid4()),
                "text": "[transcription placeholder]",
                "status": "committed",
                "start_sample": 0,
                "end_sample": len(audio_data) // 4,
            }
        ],
    })


def main():
    app = web.Application()
    app.router.add_get("/ws", handle_ws)
    app.router.add_get("/health", handle_health)
    app.router.add_post("/transcribe", handle_transcribe)

    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9091
    print(f"Starting ASR v2 Gateway on port {port}", file=sys.stderr)
    web.run_app(app, host="0.0.0.0", port=port)


if __name__ == "__main__":
    main()