# GTO Studio em container (Hugging Face Spaces ou qualquer host com Docker).
FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
RUN cargo build --release

FROM debian:bookworm-slim
# O Hugging Face roda o container com o usuário de uid 1000.
RUN useradd -m -u 1000 user
COPY --from=build /app/target/release/gto-studio /usr/local/bin/gto-studio
USER user
ENV GTO_HOST=0.0.0.0 \
    GTO_PORT=7860
EXPOSE 7860
CMD ["gto-studio"]
