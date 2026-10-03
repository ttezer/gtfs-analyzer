# Build from the repository root:
#   docker build -f plugin/mcp/Dockerfile -t gtfs-validator-mcp .

FROM rust:bookworm AS analyzer-builder
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN cargo build --release --locked -p gtfs-analyzer

FROM python:3.12-slim-bookworm
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    MCP_HOST=0.0.0.0 \
    MCP_PORT=8080 \
    GTFS_ANALYZER_BIN=/usr/local/bin/gtfs-analyzer

COPY --from=analyzer-builder /src/target/release/gtfs-analyzer /usr/local/bin/gtfs-analyzer
COPY plugin/mcp /app
WORKDIR /app
RUN pip install --no-cache-dir . \
    && useradd --create-home --uid 10001 appuser \
    && chown -R appuser:appuser /app
USER appuser
EXPOSE 8080
CMD ["gtfs-validator-mcp"]
