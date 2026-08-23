#!/usr/bin/env python3
"""sherpa-onnx Korean Zipformer streaming worker.
Receives 16kHz mono PCM chunks via WebSocket, returns partial/final captions.

Protocol: WebSocket binary (float32 PCM) → JSON CaptionEvent text
"""

import asyncio
import json
import sys
import numpy as np

# ---------------------------------------------------------------------------
# sherpa-onnx OnlineRecognizer
# ---------------------------------------------------------------------------
try:
    import sherpa_onnx
except ImportError:
    sherpa_onnx = None


def _create_recognizer(model_path: str | None = None) -> "sherpa_onnx.OnlineRecognizer":
    """Create a Zipformer Korean streaming recognizer with CUDA support."""
    if sherpa_onnx is None:
        raise RuntimeError("sherpa-onnx not installed")

    if model_path is None:
        model_path = "sherpa-onnx-streaming-zipformer-korean-2024-06-16"

    recognizer_config = sherpa_onnx.OnlineRecognizerConfig(
        feat_config=sherpa_onnx.FeatureConfig(
            sample_rate=16000,
            feature_dim=80,
        ),
        decoder_config=sherpa_onnx.OnlineDecoderConfig(
            num_threads=4,
            provider="cuda",
            model_config=sherpa_onnx.OnlineModelConfig(
                zipformer=sherpa_onnx.OnlineZipformerModelConfig(
                    model=model_path,
                    encoder="encoder-epoch-30-avg-1.int8.onnx",
                    decoder="decoder-epoch-30-avg-1.onnx",
                    joiner="joiner-epoch-30-avg-1.int8.onnx",
                ),
                tokens=f"{model_path}/tokens.txt",
                num_threads=4,
                provider="cuda",
                debug=False,
            ),
        ),
        endpoint_config=sherpa_onnx.EndpointConfig(
            rule1_min_trailing_silence=2.4,
            rule2_min_trailing_silence=1.2,
            rule3_min_utterance_length=20.0,
        ),
        enable_endpoint_detection=True,
        enable_endpoint=True,
    )
    return sherpa_onnx.OnlineRecognizer(recognizer_config)


class ZipformerWorker:
    """Per-connection Zipformer streaming ASR worker."""

    def __init__(self, model_path: str | None = None):
        self.recognizer = _create_recognizer(model_path)
        self.stream = self.recognizer.create_stream()
        self.segment_id_counter = 0
        self._buffer: list[float] = []

    def accept_waveform(self, samples: list[float], sample_rate: int = 16000):
        """Accept 16kHz mono PCM float32 samples."""
        self._buffer.extend(samples)
        # Feed 320-sample chunks (20ms at 16kHz) to match Zipformer's 320ms chunk size
        chunk_size = 5120  # 320ms at 16kHz
        while len(self._buffer) >= chunk_size:
            chunk = self._buffer[:chunk_size]
            self._buffer = self._buffer[chunk_size:]
            arr = np.array(chunk, dtype=np.float32)
            self.stream.accept_waveform(sample_rate, arr)
            self.recognizer.decode_stream(self.stream)

    def get_partial_result(self) -> dict | None:
        """Get the current partial result. Returns None if no change."""
        if not self.recognizer.is_ready(self.stream):
            return None
        text = self.recognizer.get_result(self.stream).text.strip()
        if not text:
            return None
        self.segment_id_counter += 1
        return {
            "segment_id": f"zip-{self.segment_id_counter}",
            "text": text,
            "is_final": False,
            "status": "partial",
        }

    def get_final_result(self) -> dict | None:
        """Get the final result after endpoint detection."""
        if not self.recognizer.is_endpoint(self.stream):
            return None
        text = self.recognizer.get_result(self.stream).text.strip()
        if not text:
            return None
        self.recognizer.reset(self.stream)
        self.segment_id_counter += 1
        return {
            "segment_id": f"zip-{self.segment_id_counter}",
            "text": text,
            "is_final": True,
            "status": "stable",
        }

    def finish(self):
        """Signal the end of the stream."""
        self.stream.finish()
        self.recognizer.decode_stream(self.stream)
        text = self.recognizer.get_result(self.stream).text.strip()
        self.recognizer.reset(self.stream)
        if text:
            self.segment_id_counter += 1
            return {
                "segment_id": f"zip-{self.segment_id_counter}",
                "text": text,
                "is_final": True,
                "status": "stable",
            }
        return None


# ---------------------------------------------------------------------------
# WebSocket server
# ---------------------------------------------------------------------------
try:
    from aiohttp import web
except ImportError:
    web = None


async def handle_asr(request):
    ws = web.WebSocketResponse()
    await ws.prepare(request)
    worker = ZipformerWorker()

    try:
        async for msg in ws:
            if msg.type == web.WSMsgType.BINARY:
                # Binary float32 PCM data
                raw = msg.data
                count = len(raw) // 4
                samples = list(struct.unpack(f"{count}f", raw))
                worker.accept_waveform(samples)

                # Check for partial results
                partial = worker.get_partial_result()
                if partial:
                    await ws.send_json(partial)

                # Check for endpoint (final)
                final = worker.get_final_result()
                if final:
                    await ws.send_json(final)

            elif msg.type == web.WSMsgType.TEXT:
                data = json.loads(msg.data)
                if data.get("command") == "done":
                    result = worker.finish()
                    if result:
                        await ws.send_json(result)
                    break

            elif msg.type == web.WSMsgType.ERROR:
                break
    finally:
        pass

    return ws


async def handle_health(request):
    return web.json_response({"status": "ok", "service": "zipformer"})


import struct


def main():
    app = web.Application()
    app.router.add_get("/asr", handle_asr)
    app.router.add_get("/health", handle_health)

    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9093
    print(f"Starting Zipformer ASR worker on port {port}", file=sys.stderr)
    web.run_app(app, host="0.0.0.0", port=port)


if __name__ == "__main__":
    main()