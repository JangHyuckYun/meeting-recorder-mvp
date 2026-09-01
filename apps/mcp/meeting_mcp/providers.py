import asyncio
import json
import sys
import uuid
import wave
from array import array
from pathlib import Path
from typing import Any

import httpx
import websockets


def words_to_segments(words: list[dict[str, Any]]) -> list[dict[str, Any]]:
    segments: list[dict[str, Any]] = []
    current: dict[str, Any] | None = None
    pending = ""
    for item in words:
        text = str(item.get("text", ""))
        if item.get("type") != "word":
            if current:
                current["text"] += text
            else:
                pending += text
            continue
        speaker_id = item.get("speaker_id")
        speaker = (
            f"화자 {int(str(speaker_id).rsplit('_', 1)[-1]) + 1}"
            if speaker_id is not None and str(speaker_id).rsplit("_", 1)[-1].isdigit()
            else "화자 미확인"
        )
        if not current or current["speaker"] != speaker:
            if current:
                current["text"] = current["text"].strip()
                segments.append(current)
            current = {
                "start": float(item.get("start") or 0),
                "end": float(item.get("end") or item.get("start") or 0),
                "speaker": speaker,
                "text": pending + text,
            }
            pending = ""
        else:
            current["text"] += text
            current["end"] = float(item.get("end") or current["end"])
    if current:
        current["text"] = current["text"].strip()
        segments.append(current)
    return segments


async def elevenlabs(wav_path: Path, config: dict[str, Any]) -> list[dict[str, Any]]:
    if not config.get("api_key"):
        raise ValueError("ElevenLabs is not configured: set ELEVENLABS_API_KEY")
    data = {
        "model_id": config["model_id"],
        "diarize": str(bool(config["diarize"])).lower(),
        "timestamps_granularity": "word",
    }
    if config.get("language_code"):
        data["language_code"] = str(config["language_code"])
    if isinstance(config["speakers"], int):
        data["num_speakers"] = str(config["speakers"])
    async with httpx.AsyncClient(timeout=600) as client:
        with wav_path.open("rb") as audio:
            response = await client.post(
                "https://api.elevenlabs.io/v1/speech-to-text",
                headers={"xi-api-key": config["api_key"]},
                data=data,
                files={"file": (wav_path.name, audio, "audio/wav")},
            )
        response.raise_for_status()
    return words_to_segments(response.json().get("words", []))


def _speaker(value: Any) -> str:
    if value is None:
        return "화자 미확인"
    tail = str(value).rsplit("_", 1)[-1]
    return f"화자 {int(tail) + 1}" if tail.isdigit() else "화자 미확인"


async def local(wav_path: Path, config: dict[str, Any]) -> list[dict[str, Any]]:
    if not config.get("ws_url"):
        raise ValueError("Local WhisperLive is not configured: set MEETING_MCP_LOCAL_WS_URL")
    handshake = {
        "uid": str(uuid.uuid4()),
        "language": config["language"],
        "task": "transcribe",
        "model": config["model"],
        "use_vad": True,
        "send_last_n_segments": 50,
        "no_speech_thresh": 0.7,
        "clip_audio": False,
        "same_output_threshold": 5,
        "enable_translation": False,
        "enable_diarization": True,
        # WhisperLive requires max_speakers, so auto preserves its existing four-speaker default.
        "max_speakers": 4 if config["speakers"] == "auto" else config["speakers"],
        "diarization_threshold": config["diarization_threshold"],
        "word_timestamps": False,
        "audio_format": "float32",
    }
    segments: dict[float, dict[str, Any]] = {}
    sent = asyncio.Event()
    async with websockets.connect(config["ws_url"], open_timeout=30) as ws:
        await ws.send(json.dumps(handshake))
        while True:
            ready = json.loads(await asyncio.wait_for(ws.recv(), timeout=30))
            if ready.get("status") in {"WAIT", "ERROR"}:
                raise RuntimeError(f"WhisperLive: {ready}")
            if ready.get("message") == "SERVER_READY":
                break

        async def receive() -> None:
            deadline = 0.0
            while True:
                if sent.is_set() and not deadline:
                    deadline = asyncio.get_running_loop().time() + 30
                timeout = max(0.01, min(1.0, deadline - asyncio.get_running_loop().time())) if deadline else 1.0
                try:
                    raw = await asyncio.wait_for(ws.recv(), timeout=timeout)
                except asyncio.TimeoutError:
                    if deadline and asyncio.get_running_loop().time() >= deadline:
                        return
                    continue
                except websockets.ConnectionClosed:
                    return
                if isinstance(raw, bytes):
                    continue
                message = json.loads(raw)
                if message.get("status") == "ERROR":
                    raise RuntimeError(f"WhisperLive: {message}")
                for segment in message.get("segments", []):
                    start = float(segment["start"])
                    segments[start] = {
                        "start": start,
                        "end": float(segment["end"]),
                        "speaker": _speaker(segment.get("speaker")),
                        "text": str(segment.get("text", "")).strip(),
                    }
                if deadline:
                    deadline = asyncio.get_running_loop().time() + 30

        receiver = asyncio.create_task(receive())
        with wave.open(str(wav_path), "rb") as wav:
            while pcm := wav.readframes(32768):
                samples = array("h", pcm)
                if sys.byteorder != "little":
                    samples.byteswap()
                floats = array("f", (sample / 32768.0 for sample in samples))
                if sys.byteorder != "little":
                    floats.byteswap()
                await ws.send(floats.tobytes())
                await asyncio.sleep(len(samples) / 16000 * 0.25)
        await ws.send(b"END_OF_AUDIO")
        sent.set()
        await receiver
    return [segments[start] for start in sorted(segments)]


PROVIDERS = {"elevenlabs": elevenlabs, "local": local}
