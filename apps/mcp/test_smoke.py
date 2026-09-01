import os
import tempfile
from pathlib import Path

from meeting_mcp.server import _validate_speakers, effective_config, mask_config, render_output, words_to_segments


words = [
    {"text": "안녕", "type": "word", "start": 0.1, "end": 0.5, "speaker_id": "speaker_0"},
    {"text": " ", "type": "spacing"},
    {"text": "하세요.", "type": "word", "start": 0.6, "end": 1.0, "speaker_id": "speaker_0"},
    {"text": "반갑습니다", "type": "word", "start": 1.2, "end": 2.0, "speaker_id": "speaker_1"},
]
segments = words_to_segments(words)
assert segments == [
    {"start": 0.1, "end": 1.0, "speaker": "화자 1", "text": "안녕 하세요."},
    {"start": 1.2, "end": 2.0, "speaker": "화자 2", "text": "반갑습니다"},
]

one = {"segments": [{"start": 1.234, "end": 65.678, "speaker": "화자 1", "text": "테스트"}]}
assert render_output("srt", "meeting", one) == "1\n00:00:01,234 --> 00:01:05,678\n화자 1: 테스트\n\n"
assert render_output("txt", "meeting", one) == "[00:00:01] 화자 1: 테스트"

with tempfile.TemporaryDirectory() as temporary:
    path = Path(temporary) / "config.json"
    path.write_text('{"elevenlabs":{"model_id":"saved"},"local":{"ws_url":"ws://saved"}}')
    env = {"ELEVENLABS_API_KEY": "sk_test_1234", "MEETING_MCP_LOCAL_WS_URL": "ws://env"}
    config = effective_config(path, env)
    assert config["elevenlabs"]["model_id"] == "saved"
    assert config["elevenlabs"]["speakers"] == "auto"
    assert config["local"]["speakers"] == 4
    assert config["local"]["ws_url"] == "ws://env"
    assert mask_config(config["elevenlabs"])["api_key"] == "sk_…1234"
    assert oct(os.stat(path).st_mode & 0o777) == "0o600"

assert _validate_speakers("auto") == "auto"
assert _validate_speakers(2) == 2
for invalid in (0, -1, "2", True):
    try:
        _validate_speakers(invalid)
        assert False, invalid
    except ValueError:
        pass

print("smoke tests passed")
