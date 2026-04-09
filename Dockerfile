# Stage 1: Build Stage
FROM rust:1-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./

# Cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

# --- NEU: Conditional Models Stages ---
# Diese Stage wird genutzt, wenn INCLUDE_MODELS=true
FROM alpine:latest AS models-true
# Wir nutzen den Wildcard-Trick, damit der Build nicht abbricht, falls der Ordner fehlt
COPY models* /models/

# Diese Stage wird genutzt, wenn INCLUDE_MODELS=false (Standard)
FROM alpine:latest AS models-false
RUN mkdir /models

# Hier wird basierend auf dem Argument entschieden, welche Stage als Quelle dient
ARG INCLUDE_MODELS=false
FROM models-${INCLUDE_MODELS} AS models-source

# Stage 2: Runtime Stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Binärdatei kopieren
COPY --from=builder /app/target/release/deckblattscanner-backend /app/scanner-backend

# Models aus der gewählten Source-Stage kopieren
COPY --from=models-source /models /app/models/

# Sicherstellen, dass das Verzeichnis existiert (falls models-false gewählt wurde)
RUN mkdir -p /app/models

EXPOSE 3000
ENV RUST_LOG=info
ENV PORT=3000

CMD ["/app/scanner-backend"]
