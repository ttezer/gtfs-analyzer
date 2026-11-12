# Build from the repository root:
#   docker build --build-arg GTFS_ANALYZER_COMMIT="$(git rev-parse HEAD)" \
#     -f plugin/mcp/Dockerfile -t gtfs-validator-mcp .

# Base images are pinned by digest (multi-arch index) so a changed upstream tag cannot
# alter the public Cloud Run runtime. To update: resolve the tag's current digest and
# replace both the comment and the digest.
# rust:bookworm @ 2026-11-12
FROM rust:bookworm@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c AS analyzer-builder
WORKDIR /src
ARG GTFS_ANALYZER_COMMIT
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN GTFS_ANALYZER_COMMIT="$GTFS_ANALYZER_COMMIT" cargo build --release --locked -p gtfs-analyzer

# python:3.12-slim-bookworm @ 2026-11-12
FROM python:3.12-slim-bookworm@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    MCP_HOST=0.0.0.0 \
    MCP_PORT=8080 \
    GTFS_ANALYZER_BIN=/usr/local/bin/gtfs-analyzer

COPY --from=analyzer-builder /src/target/release/gtfs-analyzer /usr/local/bin/gtfs-analyzer
COPY plugin/mcp /app
WORKDIR /app
# Every Python dependency (and the setuptools build backend) comes from the hash-locked
# requirements.lock; the project itself is then installed without resolving anything.
# Regenerate the lock with the command in its header after changing pyproject.toml.
RUN pip install --no-cache-dir --require-hashes --only-binary=:all: -r requirements.lock \
    && pip install --no-cache-dir --no-deps --no-build-isolation . \
    && useradd --create-home --uid 10001 appuser \
    && chown -R appuser:appuser /app
USER appuser
EXPOSE 8080
CMD ["gtfs-validator-mcp"]
