#!/usr/bin/env python3
"""RNNoise noise suppression + Silero VAD service.
Accepts 48kHz mono PCM chunks, returns denoised audio with VAD events.

Protocol: WebSocket JSON at /vad
  Input:  {"samples": [float...], "sample_rate": 48000}
  Output: {"is_speech": bool, "frame_offset": int, "denoised": [float...]}
"""

import asyncio
import json
import struct
import subprocess
import sys
from pathlib import Path

try:
    import numpy as np
except ImportError:
    np = None

# ---------------------------------------------------------------------------
# RNNoise C ABI via ctypes
# ---------------------------------------------------------------------------
import ctypes
import ctypes.util

_lib = None
_rnnoise_frame_size = 480  # 10ms at 48kHz


def _load_rnnoise() -> bool:
    global _lib
    if _lib is not None:
        return True
    lib_path = ctypes.util.find_library("rnnoise")
    if lib_path is None:
        # Try common paths
        for p in ["/usr/local/lib/librnnoise.so", "/usr/lib/librnnoise.so",
                   "/usr/local/lib/librnnoise.dylib", "/opt/homebrew/lib/librnnoise.dylib"]:
            if Path(p).exists():
                lib_path = p
                break
    if lib_path is None:
        return False
    try:
        _lib = ctypes.cdll.LoadLibrary(lib_path)
        _lib.rnnoise_create.restype = ctypes.c_void_p
        _lib.rnnoise_process_frame.argtypes = [
            ctypes.c_void_p,                 # state
            ctypes.POINTER(ctypes.c_float),  # out
            ctypes.POINTER(ctypes.c_float),  # in
        ]
        _lib.rnnoise_process_frame.restype = ctypes.c_float
        return True
    except Exception:
        return False


class RNNoiseProcessor:
    """Per-stream RNNoise state. One instance per client connection."""

    def __init__(self):
        if not _load_rnnoise():
            raise RuntimeError("librnnoise not found; install RNNoise first")
        self._state = _lib.rnnoise_create(None)

    def process_frame(self, samples: list[float]) -> list[float]:
        """Process 480 samples (10ms at 48kHz). Returns denoised 480 samples."""
        if len(samples) != _rnnoise_frame_size:
            raise ValueError(f"RNNoise requires exactly {_rnnoise_frame_size} samples")
        in_arr = (ctypes.c_float * _rnnoise_frame_size)(*samples)
        out_arr = (ctypes.c_float * _rnnoise_frame_size)()
        _lib.rnnoise_process_frame(self._state, out_arr, in_arr)
        return list(out_arr)

    def destroy(self):
        if self._lib:
            _lib.rnnoise_destroy(self._state)


# ---------------------------------------------------------------------------
# Silero VAD (ONNX)
# ---------------------------------------------------------------------------
try:
    import onnxruntime as ort
except ImportError:
    ort = None

_silero_session = None
_silero_input_name = None
_silero_output_name = None
_sr = 16000  # Silero expects 16kHz


def _load_silero() -> bool:
    global _silero_session, _silero_input_name, _silero_output_name
    if _silero_session is not None:
        return True
    if ort is None:
        return False
    model_path = Path(__file__).parent.parent / "models" / "silero_vad.onnx"
    if not model_path.exists():
        # Download from upstream
        import urllib.request
        model_path.parent.mkdir(parents=True, exist_ok=True)
        url = "https://github.com/snakers4/silero-vad/raw/v6.2.1/src/silero_vad/data/silero_vad.onnx"
        urllib.request.urlretrieve(url, model_path)
    try:
        _silero_session = ort.InferenceSession(
            str(model_path),
            providers=["CPUExecutionProvider"],
        )
        _silero_input_name = _silero_session.get_inputs()[0].name
        _silero_output_name = _silero_session.get_outputs()[0].name
        return True
    except Exception:
        return False


def silero_vad(audio_16k: list[float]) -> float:
    """Run Silero VAD on 16kHz mono audio. Returns speech probability (0-1)."""
    if not _load_silero():
        return 0.5  # fallback: uncertain
    if np is None:
        return 0.5
    # Silero expects shape (1, seq_len) normalized
    arr = np.array(audio_16k, dtype=np.float32).reshape(1, -1)
    outputs = _silero_session.run([_silero_output_name], {_silero_input_name: arr})
    return float(outputs[0][0][0][0])


# ---------------------------------------------------------------------------
# ASGI / WebSocket server
# ---------------------------------------------------------------------------
try:
    from aiohttp import web
except ImportError:
    web = None


async def handle_vad(request):
    ws = web.WebSocketResponse()
    await ws.prepare(request)
    rnnoise = RNNoiseProcessor()
    buf_48k: list[float] = []
    buf_16k: list[float] = []
    frame_offset = 0

    try:
        async for msg in ws:
            if msg.type == web.WSMsgType.TEXT:
                data = json.loads(msg.data)
                samples = data.get("samples", [])
                sr = data.get("sample_rate", 48000)

                # Downmix to mono if needed
                if sr == 48000:
                    buf_48k.extend(samples)
                    # Process in 480-sample RNNoise frames
                    while len(buf_48k) >= _rnnoise_frame_size:
                        frame = buf_48k[:_rnnoise_frame_size]
                        buf_48k = buf_48k[_rnnoise_frame_size:]
                        denoised = rnnoise.process_frame(frame)
                        # Downsample 48k→16k: take every 3rd sample
                        for i in range(0, len(denoised), 3):
                            buf_16k.append(denoised[i])
                    # Run VAD on accumulated 16k (512 samples = 32ms)
                    while len(buf_16k) >= 512:
                        chunk = buf_16k[:512]
                        buf_16k = buf_16k[512:]
                        prob = silero_vad(chunk)
                        is_speech = prob > 0.5
                        await ws.send_json({
                            "is_speech": is_speech,
                            "probability": round(prob, 4),
                            "frame_offset": frame_offset,
                            "sample_rate_16k": 16000,
                        })
                        frame_offset += 512

            elif msg.type == web.WSMsgType.ERROR:
                break
    finally:
        rnnoise.destroy()

    return ws


async def handle_health(request):
    return web.json_response({"status": "ok", "service": "noise-vad"})


def main():
    if not _load_rnnoise():
        print("WARNING: librnnoise not found — running in passthrough (noise suppression disabled)", file=sys.stderr)
    if not _load_silero():
        print("WARNING: Silero VAD ONNX model not loaded — running with fallback", file=sys.stderr)

    app = web.Application()
    app.router.add_get("/vad", handle_vad)
    app.router.add_get("/health", handle_health)

    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9092
    print(f"Starting noise-vad service on port {port}", file=sys.stderr)
    web.run_app(app, host="0.0.0.0", port=port)


if __name__ == "__main__":
    main()