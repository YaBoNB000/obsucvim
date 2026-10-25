FROM rust:1.88-bookworm AS engine
WORKDIR /build
COPY engine/Cargo.toml engine/Cargo.lock ./
COPY engine/src ./src
RUN cargo build --release --locked --offline

FROM python:3.12-slim-bookworm
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1
WORKDIR /app
COPY services/discord_obf/requirements.txt ./requirements.txt
RUN pip install --no-cache-dir -r requirements.txt \
    && useradd --uid 10001 --create-home appuser
COPY --from=engine /build/target/release/obsucvim /usr/local/bin/obsucvim
COPY services/discord_obf ./services/discord_obf
USER appuser
EXPOSE 10000
# ONE web worker: queue / dedup / per-user locks are in memory. No --preload.
CMD ["sh", "-c", "exec gunicorn 'services.discord_obf.app:create_app()' --bind 0.0.0.0:${PORT:-10000} --workers 1 --threads 4 --timeout 30 --graceful-timeout 30"]
