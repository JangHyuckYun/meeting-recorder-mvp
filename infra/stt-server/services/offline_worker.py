#!/usr/bin/env python3
"""Offline finalization worker.
After meeting Done signal, runs pyannote speaker correction + large-v3 re-decode + export.

Protocol: HTTP POST /finalize with WAV file → JSON/MD/SRT exports
"""
import asyncio
import json
import struct
import sys
import uuid
from io import BytesIO
from pathlib import Path

try:
    import numpy as np
except ImportError:
    np = None


class OfflineFinalizer:
    """Offline meeting finalization: diarization → re-ASR → export."""

    def __init__(self, device: str = "cuda"):
        self._diarization = None
        self._asr_model = None
        self._device = device

    def _load_diarization(self):
        if self._diarization is not None:
            return
        try:
            from pyannote.audio import Pipeline
            pipeline = Pipeline.from_pretrained(
                "pyannote/speaker-diarization-community-1",
                use_auth_token=None,
            )
            # Pin to local model path
            self._diarization = pipeline
            print("pyannote diarization pipeline loaded", file=sys.stderr)
        except Exception as e:
            print(f"WARNING: Could not load pyannote: {e}", file=sys.stderr)
            self._diarization = None

    def _load_asr(self):
        if self._asr_model is not None:
            return
        try:
            from faster_whisper import WhisperModel
            self._asr_model = WhisperModel(
                "large-v3",
                device=self._device,
                compute_type="int8_float16",
            )
            print(f"Offline large-v3 model loaded on {self._device}", file=sys.stderr)
        except Exception as e:
            print(f"WARNING: Could not load large-v3: {e}", file=sys.stderr)
            self._asr_model = None

    def finalize(self, audio_16k: list[float], language: str = "ko") -> dict:
        """Run full offline finalization. Returns dict with transcripts and exports."""
        self._load_diarization()
        self._load_asr()

        if np is None:
            return {"error": "numpy not available"}

        arr = np.array(audio_16k, dtype=np.float32)

        # 1. Run diarization (if available)
        speaker_segments = []
        if self._diarization is not None:
            try:
                diarization = self._diarization({"waveform": arr[None, :], "sample_rate": 16000})
                for turn, _, speaker in diarization.itertracks(yield_label=True):
                    speaker_segments.append({
                        "speaker": speaker,
                        "start": turn.start,
                        "end": turn.end,
                    })
            except Exception as e:
                print(f"Diarization error: {e}", file=sys.stderr)

        # 2. Run large-v3 ASR
        transcript_segments = []
        if self._asr_model is not None:
            try:
                raw_segments, info = self._asr_model.transcribe(
                    arr, language=language, beam_size=5, word_timestamps=False
                )
                for i, seg in enumerate(raw_segments):
                    transcript_segments.append({
                        "segment_id": str(uuid.uuid4()),
                        "text": seg.text.strip(),
                        "start_sample": int(seg.start * 16000),
                        "end_sample": int(seg.end * 16000),
                        "speaker_label": None,
                    })
            except Exception as e:
                print(f"ASR error: {e}", file=sys.stderr)

        # 3. Align speaker labels with transcript segments
        for seg in transcript_segments:
            seg_start = seg["start_sample"] / 16000
            seg_end = seg["end_sample"] / 16000
            for spk in speaker_segments:
                if spk["start"] <= seg_start <= spk["end"]:
                    seg["speaker_label"] = spk["speaker"]
                    break

        # 4. Generate exports
        srt_content = self._generate_srt(transcript_segments)
        json_content = json.dumps(transcript_segments, ensure_ascii=False, indent=2)
        md_content = self._generate_md(transcript_segments)

        return {
            "segments": transcript_segments,
            "speaker_segments": speaker_segments,
            "exports": {
                "srt": srt_content,
                "json": json_content,
                "md": md_content,
            },
        }

    def _generate_srt(self, segments: list[dict]) -> str:
        lines = []
        for i, seg in enumerate(segments, 1):
            start_ms = int(seg["start_sample"] / 16000 * 1000)
            end_ms = int(seg["end_sample"] / 16000 * 1000)
            start_srt = f"{start_ms // 3600000:02d}:{(start_ms % 3600000) // 60000:02d}:{(start_ms % 60000) // 1000:02d},{start_ms % 1000:03d}"
            end_srt = f"{end_ms // 3600000:02d}:{(end_ms % 3600000) // 60000:02d}:{(end_ms % 60000) // 1000:02d},{end_ms % 1000:03d}"
            speaker = f"[{seg.get('speaker_label', '')}] " if seg.get("speaker_label") else ""
            lines.append(f"{i}\n{start_srt} --> {end_srt}\n{speaker}{seg['text']}\n")
        return "\n".join(lines)

    def _generate_md(self, segments: list[dict]) -> str:
        lines = ["# 회의록\n", f"생성 시간: {__import__('datetime').datetime.now().isoformat()}\n", "---\n"]
        for seg in segments:
            speaker = f"**{seg.get('speaker_label', '화자')}**" if seg.get("speaker_label") else "**화자**"
            lines.append(f"{speaker}: {seg['text']}\n")
        return "\n".join(lines)


# ---------------------------------------------------------------------------
# HTTP server
# ---------------------------------------------------------------------------
try:
    from aiohttp import web
except ImportError:
    web = None


async def handle_finalize(request):
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
    audio_bytes = b"".join(chunks)

    # Decode WAV or raw PCM
    # Simple approach: if it starts with RIFF, parse WAV header; otherwise treat as raw PCM
    language = request.query.get("language", "ko")
    finalizer = OfflineFinalizer()

    if audio_bytes[:4] == b"RIFF":
        # Simple WAV parser (mono 16-bit PCM)
        import wave
        with io.BytesIO(audio_bytes) as buf:
            with wave.open(buf, "rb") as wav:
                frames = wav.readframes(wav.getnframes())
                samples = list(struct.unpack(f"{len(frames) // 2}h", frames))
                audio_16k = [s / 32768.0 for s in samples]
    else:
        count = len(audio_bytes) // 4
        audio_16k = list(struct.unpack(f"{count}f", audio_bytes))

    result = finalizer.finalize(audio_16k, language=language)
    return web.json_response(result)


async def handle_health(request):
    return web.json_response({"status": "ok", "service": "offline"})


import io


def main():
    app = web.Application()
    app.router.add_post("/finalize", handle_finalize)
    app.router.add_get("/health", handle_health)

    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9097
    print(f"Starting offline finalization worker on port {port}", file=sys.stderr)
    web.run_app(app, host="0.0.0.0", port=port)


if __name__ == "__main__":
    main()