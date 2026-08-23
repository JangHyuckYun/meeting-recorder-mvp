#!/usr/bin/env python3
"""MossFormer2 overlap separation worker.
Receives overlapping audio chunks, returns separated speaker streams.
Non-causal — only used for delayed REVISED work, never for live PARTIAL.

Protocol: WebSocket JSON at /separate
  Input:  {"audio_base64": "...", "sample_rate": 16000}
  Output: {"streams": [{"speaker": 1, "audio_base64": "..."}, ...]}
"""
import asyncio
import base64
import io
import json
import struct
import sys

try:
    import numpy as np
except ImportError:
    np = None


class MossFormerWorker:
    """MossFormer2 two-speaker overlap separator."""

    def __init__(self, device: str = "cuda"):
        self._model = None
        self._device = device

    def _load_model(self):
        if self._model is not None:
            return
        try:
            import torch
            # MossFormer2 from ClearerVoice-Studio
            from clearer_voice.models import MossFormer2_SS_16K
            self._model = MossFormer2_SS_16K.from_pretrained(
                "alibabasglab/MossFormer2_SS_16K"
            )
            self._model = self._model.to(self._device)
            self._model.eval()
            print(f"MossFormer2 model loaded on {self._device}", file=sys.stderr)
        except ImportError:
            print("WARNING: ClearerVoice-Studio not installed. MossFormer unavailable.", file=sys.stderr)
            self._model = None
        except Exception as e:
            print(f"WARNING: Could not load MossFormer2: {e}", file=sys.stderr)
            self._model = None

    def separate(self, audio_16k: list[float]) -> list[list[float]]:
        """Separate a 16kHz mono mixture into two speaker streams."""
        self._load_model()
        if self._model is None or np is None:
            return [audio_16k, [0.0] * len(audio_16k)]  # passthrough

        import torch
        arr = np.array(audio_16k, dtype=np.float32)
        # Model expects shape (1, T) or (T,)
        with torch.no_grad():
            inp = torch.from_numpy(arr[None, :]).to(self._device)
            outputs = self._model(inp)  # returns (2, T) or list of 2 tensors

        if isinstance(outputs, torch.Tensor):
            separated = outputs.cpu().numpy()
        else:
            separated = np.array([o.cpu().numpy() for o in outputs])

        stream1 = separated[0].tolist() if separated.shape[0] > 0 else audio_16k
        stream2 = separated[1].tolist() if separated.shape[0] > 1 else [0.0] * len(audio_16k)
        return [stream1, stream2]


# ---------------------------------------------------------------------------
# WebSocket server
# ---------------------------------------------------------------------------
try:
    from aiohttp import web
except ImportError:
    web = None


async def handle_separate(request):
    ws = web.WebSocketResponse()
    await ws.prepare(request)
    worker = MossFormerWorker()

    try:
        async for msg in ws:
            if msg.type == web.WSMsgType.TEXT:
                data = json.loads(msg.data)
                command = data.get("command", "")

                if command == "separate":
                    audio_b64 = data.get("audio_base64", "")
                    audio_bytes = base64.b64decode(audio_b64)
                    count = len(audio_bytes) // 4
                    samples = list(struct.unpack(f"{count}f", audio_bytes))
                    streams = worker.separate(samples)

                    response = {
                        "type": "separated",
                        "streams": [
                            {"speaker": i + 1,
                             "audio_base64": base64.b64encode(
                                 struct.pack(f"{len(s)}f", *s)
                             ).decode()}
                            for i, s in enumerate(streams)
                        ],
                    }
                    await ws.send_json(response)

                elif command == "done":
                    break

            elif msg.type == web.WSMsgType.ERROR:
                break
    finally:
        pass

    return ws


async def handle_health(request):
    return web.json_response({"status": "ok", "service": "mossformer"})


def main():
    app = web.Application()
    app.router.add_get("/separate", handle_separate)
    app.router.add_get("/health", handle_health)

    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9096
    print(f"Starting MossFormer2 overlap worker on port {port}", file=sys.stderr)
    web.run_app(app, host="0.0.0.0", port=port)


if __name__ == "__main__":
    main()