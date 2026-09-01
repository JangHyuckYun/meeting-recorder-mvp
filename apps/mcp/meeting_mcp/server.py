import asyncio
import json
import os
import shutil
import stat
import subprocess
import tempfile
import wave
from pathlib import Path
from typing import Any

from mcp.server.fastmcp import FastMCP

from meeting_mcp.providers import PROVIDERS, words_to_segments

ROOT = Path(__file__).resolve().parents[1]
CONFIG_PATH = Path.home() / ".config/meeting-mcp/config.json"
DEFAULTS = {
    "elevenlabs": {
        "api_key": "",
        "model_id": "scribe_v2",
        "language_code": "ko",
        "diarize": True,
        "speakers": "auto",
    },
    "local": {
        "ws_url": "ws://192.168.1.189:9090",
        "model": "large-v3-turbo",
        "language": "ko",
        "speakers": 4,
        "diarization_threshold": 0.55,
    },
}
ENV_KEYS = {
    "ELEVENLABS_API_KEY": ("elevenlabs", "api_key"),
    "MEETING_MCP_LOCAL_WS_URL": ("local", "ws_url"),
}


def load_dotenv(path: Path = ROOT / ".env") -> None:
    if not path.is_file():
        return
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip() and not line.lstrip().startswith("#") and "=" in line:
            key, value = line.split("=", 1)
            os.environ.setdefault(key.strip(), value.strip().strip("\"'"))


load_dotenv()


def _read_config(path: Path = CONFIG_PATH) -> dict[str, Any]:
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        path.write_text("{}\n", encoding="utf-8")
    path.chmod(stat.S_IRUSR | stat.S_IWUSR)
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as exc:
        raise ValueError(f"Invalid config file {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise ValueError(f"Invalid config file {path}: root must be an object")
    return data


def _write_config(data: dict[str, Any], path: Path = CONFIG_PATH) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    path.chmod(stat.S_IRUSR | stat.S_IWUSR)


def effective_config(
    path: Path = CONFIG_PATH, environ: dict[str, str] | os._Environ[str] | None = None
) -> dict[str, Any]:
    saved = _read_config(path)
    config: dict[str, Any] = {
        "default_provider": saved.get("default_provider", "elevenlabs"),
        "output_dir": saved.get("output_dir"),
    }
    for name, defaults in DEFAULTS.items():
        configured = saved.get(name, {})
        if configured is not None and not isinstance(configured, dict):
            raise ValueError(f"Invalid config for provider: {name}")
        config[name] = defaults | (configured or {})
    for env_name, (provider, key) in ENV_KEYS.items():
        value = (os.environ if environ is None else environ).get(env_name)
        if value is not None:
            config[provider][key] = value
    return config


def mask_config(config: dict[str, Any]) -> dict[str, Any]:
    masked = dict(config)
    if "api_key" in masked and masked["api_key"]:
        masked["api_key"] = f"sk_…{str(masked['api_key'])[-4:]}"
    return masked


def _validate_speakers(value: Any) -> int | str:
    if value == "auto" or (isinstance(value, int) and not isinstance(value, bool) and value >= 1):
        return value
    raise ValueError('speakers must be "auto" or a positive integer')


def _clock(seconds: float, milliseconds: bool = False) -> str:
    total = max(0, round(seconds * 1000) if milliseconds else int(seconds))
    if milliseconds:
        whole, ms = divmod(total, 1000)
    else:
        whole, ms = total, 0
    hours, remainder = divmod(whole, 3600)
    minutes, secs = divmod(remainder, 60)
    return f"{hours:02}:{minutes:02}:{secs:02}{f',{ms:03}' if milliseconds else ''}"


def render_output(fmt: str, title: str, result: dict[str, Any]) -> str:
    segments = result["segments"]
    if fmt == "srt":
        return "".join(
            f"{i}\n{_clock(s['start'], True)} --> {_clock(s['end'], True)}\n{s['speaker']}: {s['text']}\n\n"
            for i, s in enumerate(segments, 1)
        )
    if fmt == "txt":
        return "\n".join(f"[{_clock(s['start'])}] {s['speaker']}: {s['text']}" for s in segments)
    if fmt == "md":
        return f"# {title}\n\n" + "".join(
            f"**{s['speaker']}** `{_clock(s['start'])}`\n\n{s['text']}\n\n" for s in segments
        )
    return json.dumps(result, ensure_ascii=False, indent=2) + "\n"


def _wav(input_path: Path, output_path: Path) -> float:
    ffmpeg = shutil.which("ffmpeg") or ("/usr/local/bin/ffmpeg" if Path("/usr/local/bin/ffmpeg").is_file() else None)
    if ffmpeg:
        command = [ffmpeg, "-y", "-i", str(input_path), "-ac", "1", "-ar", "16000", "-sample_fmt", "s16", str(output_path)]
    elif shutil.which("afconvert"):
        command = [shutil.which("afconvert"), "-f", "WAVE", "-d", "LEI16@16000", "-c", "1", str(input_path), str(output_path)]
    else:
        raise RuntimeError("ffmpeg or afconvert is required")
    subprocess.run(command, check=True, capture_output=True)
    with wave.open(str(output_path), "rb") as wav:
        return wav.getnframes() / wav.getframerate()


mcp = FastMCP("meeting-transcribe")


@mcp.tool()
def list_providers() -> dict[str, Any]:
    config = effective_config()
    return {
        "default_provider": config["default_provider"],
        "providers": [
            {
                "name": name,
                "ready": bool(values.get("api_key" if name == "elevenlabs" else "ws_url")),
                "config": mask_config(values),
            }
            for name, values in ((name, config[name]) for name in PROVIDERS)
        ],
    }


@mcp.tool()
def configure_provider(provider: str, settings: dict[str, Any]) -> dict[str, Any]:
    saved = _read_config()
    if provider == "default":
        unknown = set(settings) - {"provider", "output_dir"}
        if unknown:
            raise ValueError(f"Unknown settings: {sorted(unknown)}")
        if "provider" in settings and settings["provider"] not in PROVIDERS:
            raise ValueError(f"Unknown provider: {settings['provider']}")
        if "provider" in settings:
            saved["default_provider"] = settings["provider"]
        if "output_dir" in settings:
            saved["output_dir"] = settings["output_dir"]
    elif provider in PROVIDERS:
        unknown = set(settings) - set(DEFAULTS[provider])
        if unknown:
            raise ValueError(f"Unknown settings for {provider}: {sorted(unknown)}")
        if "speakers" in settings:
            _validate_speakers(settings["speakers"])
        saved[provider] = (saved.get(provider) or {}) | settings
    else:
        raise ValueError(f"Unknown provider: {provider}")
    _write_config(saved)
    config = effective_config()
    return (
        {"default_provider": config["default_provider"], "output_dir": config["output_dir"]}
        if provider == "default"
        else {"provider": provider, "config": mask_config(config[provider])}
    )


@mcp.tool()
async def transcribe(
    file_path: str,
    provider: str | None = None,
    language: str | None = None,
    speakers: int | str | None = None,
    output_dir: str | None = None,
    formats: list[str] = ["md", "srt", "txt", "json"],
) -> dict[str, Any]:
    source = Path(file_path).expanduser().resolve()
    if not source.is_file():
        raise ValueError(f"Audio file does not exist or is not a file: {source}")
    if speakers is not None:
        _validate_speakers(speakers)
    unknown_formats = set(formats) - {"md", "srt", "txt", "json"}
    if unknown_formats or not formats:
        raise ValueError(f"Unsupported formats: {sorted(unknown_formats)}")
    config = effective_config()
    chosen = provider or config["default_provider"]
    if chosen not in PROVIDERS:
        raise ValueError(f"Unknown provider: {chosen}")
    provider_config = dict(config[chosen])
    provider_config["speakers"] = _validate_speakers(
        provider_config["speakers"] if speakers is None else speakers
    )
    if language is not None:
        provider_config["language_code" if chosen == "elevenlabs" else "language"] = language
    with tempfile.TemporaryDirectory(prefix="meeting-mcp-") as temporary:
        wav_path = Path(temporary) / "audio.wav"
        duration = await asyncio.to_thread(_wav, source, wav_path)
        segments = await PROVIDERS[chosen](wav_path, provider_config)
    result = {
        "provider": chosen,
        "audio_file": str(source),
        "duration_sec": duration,
        "segments": segments,
    }
    destination = Path(output_dir or config["output_dir"] or source.parent).expanduser().resolve()
    destination.mkdir(parents=True, exist_ok=True)
    output_files: dict[str, str] = {}
    for fmt in formats:
        path = destination / f"{source.stem}.{fmt}"
        suffix = 1
        while path.exists():
            path = destination / f"{source.stem}-{suffix}.{fmt}"
            suffix += 1
        path.write_text(render_output(fmt, source.stem, result), encoding="utf-8")
        output_files[fmt] = str(path)
    txt = render_output("txt", source.stem, result)
    return {
        "provider": chosen,
        "audio_file": str(source),
        "duration_sec": duration,
        "speaker_count": len({s["speaker"] for s in segments}),
        "segment_count": len(segments),
        "output_files": output_files,
        "preview": txt[:1500],
    }


def main() -> None:
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
