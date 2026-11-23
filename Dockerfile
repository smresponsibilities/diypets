# Headless stdio MCP server image for DIYPets (`diypets mcp`)
FROM rust:1-bookworm AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    libgtk-3-dev libayatana-appindicator3-dev libxdo-dev \
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libxkbcommon-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build --release --bin diypets

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libgtk-3-0 libayatana-appindicator3-1 libxdo3 \
    libxcb-render0 libxcb-shape0 libxcb-xfixes0 libxkbcommon0 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/diypets /usr/local/bin/diypets
ENTRYPOINT ["diypets"]
CMD ["mcp"]
