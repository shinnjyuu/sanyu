# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS model-files
WORKDIR /build
COPY scripts/fetch-models.sh scripts/firefox-models.txt ./
RUN sh ./fetch-models.sh /models

FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN cargo build --release --locked \
    && mkdir -p /cargo-licenses \
    && cd "$CARGO_HOME/registry/src" \
    && find . -type f \( -iname 'license*' -o -iname 'copying*' -o -iname '*notice*' \) \
        -exec cp --parents -t /cargo-licenses {} +

FROM debian:bookworm-slim
COPY --from=build --chmod=0555 /build/target/release/sanyu-translate /usr/local/bin/sanyu-translate
COPY --from=model-files /models/ /usr/share/sanyu/models/
COPY --from=build /cargo-licenses/ /usr/share/sanyu/licenses/cargo/
COPY LICENSE THIRD_PARTY_NOTICES.md /usr/share/sanyu/licenses/
COPY scripts/firefox-models.txt /usr/share/sanyu/licenses/scripts/
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/sanyu-translate"]
