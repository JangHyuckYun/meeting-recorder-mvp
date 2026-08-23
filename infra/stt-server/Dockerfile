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

# ---------------------------------------------------------------------------
# ASR v2 dependencies
# ---------------------------------------------------------------------------

# Install system dependencies for RNNoise
RUN apt-get update && apt-get install -y --no-install-recommends \
      autoconf \
      automake \
      libtool \
      pkg-config \
      build-essential \
    && rm -rf /var/lib/apt/lists/*

# Build RNNoise from source (pinned revision)
RUN git clone https://github.com/xiph/rnnoise.git /tmp/rnnoise \
    && cd /tmp/rnnoise \
    && git checkout 70f1d256acd4b34a572f999a05c87bf00b67730d \
    && ./autogen.sh \
    && ./configure --prefix=/usr/local \
    && make -j$(nproc) \
    && make install \
    && ldconfig \
    && rm -rf /tmp/rnnoise

# Install Python service dependencies
COPY requirements.txt /tmp/requirements.txt
RUN pip install --no-cache-dir -r /tmp/requirements.txt

# Download Silero VAD model
RUN mkdir -p /opt/models && python3 -c "import urllib.request; u='https://github.com/snakers4/silero-vad/raw/v6.2.1/src/silero_vad/data/silero_vad.onnx'; urllib.request.urlretrieve(u, '/opt/models/silero_vad.onnx')" && echo 'Silero VAD model downloaded'

# Download sherpa-onnx Korean Zipformer model using heredoc
RUN mkdir -p /opt/models/sherpa-zipformer-korean
RUN python3 <<'PYEOF'
import urllib.request, tarfile, tempfile, os
url = 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-korean-2024-06-16.tar.bz2'
with tempfile.NamedTemporaryFile(suffix='.tar.bz2', delete=False) as f:
    urllib.request.urlretrieve(url, f.name)
    with tarfile.open(f.name, 'r:bz2') as tar:
        base = '/opt/models/sherpa-zipformer-korean'
        for member in tar.getmembers():
            member.name = os.path.basename(member.name)
            if member.name:
                tar.extract(member, base)
    os.unlink(f.name)
print('Korean Zipformer model downloaded')
PYEOF

# Copy service scripts
COPY services/ /opt/services/

# Default entry point (can be overridden per service)
CMD ["python", "/opt/services/asr_gateway.py"]