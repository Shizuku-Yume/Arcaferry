# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.92
ARG PYTHON_VERSION=3.12

FROM rust:${RUST_VERSION}-bookworm AS rust-builder
WORKDIR /app/src-tauri

RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY src-tauri/ ./
RUN cargo build --release --bin server


FROM debian:bookworm-slim AS runtime-base
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=rust-builder /app/src-tauri/target/release/server /usr/local/bin/arcaferry-server

ENV ARCAFERRY_PORT=17236
EXPOSE 17236

ENTRYPOINT ["arcaferry-server"]


FROM runtime-base AS slim


FROM python:${PYTHON_VERSION}-slim-bookworm AS full
ENV PIP_DISABLE_PIP_VERSION_CHECK=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    ARCAFERRY_PORT=17236 \
    ARCAFERRY_SIDECAR_SCRIPT_PATH=/app/scripts/extract_hidden.py

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

COPY scripts/requirements.txt /tmp/requirements.txt
RUN pip install --no-cache-dir -r /tmp/requirements.txt \
    && python -m playwright install --with-deps firefox \
    && rm -f /tmp/requirements.txt

COPY scripts/ /app/scripts/
COPY --from=rust-builder /app/src-tauri/target/release/server /usr/local/bin/arcaferry-server

EXPOSE 17236
ENTRYPOINT ["arcaferry-server"]
