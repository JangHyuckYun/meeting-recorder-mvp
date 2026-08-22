# WhisperLive 0.9.0 image built from upstream commit
# bde730b81bb23b5286470524d5f527caf6a8b9e0.
FROM ghcr.io/collabora/whisperlive-gpu@sha256:fb95a19a7cb13839d074d6f94d3e546bfc92014a9f4dee24f153de208fe456cc

# WhisperLive treats diarization as an optional dependency. Keep the last v3
# release because its Model.from_pretrained API matches WhisperLive 0.9.0.
# Pyannote 3 uses torchaudio.AudioMetaData, which was removed after 2.8.
RUN pip install --no-cache-dir \
      "pyannote.audio==3.4.0" \
      "torchaudio==2.8.0" \
    && python -c "from pyannote.audio import Inference, Model"
