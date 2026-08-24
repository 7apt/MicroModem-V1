FROM rust:1.95-bookworm

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    file \
    libgl1-mesa-dev \
    libwayland-dev \
    libx11-dev libxi-dev libxcursor-dev libxrandr-dev libxkbcommon-dev \
    patchelf \
    wget \
    xdg-utils \
 && rm -rf /var/lib/apt/lists/*

RUN wget -qO /usr/local/bin/appimagetool \
    https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage \
 && chmod 0755 /usr/local/bin/appimagetool

WORKDIR /work
