FROM rust:1.98-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY build.rs ./
COPY src ./src
ARG GIT_SHA=dev
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    GIT_SHA="$GIT_SHA" cargo build --release --locked && cp target/release/seol-gitops-smoke /smoke

FROM scratch
COPY --from=build /smoke /smoke
USER 65534:65534
EXPOSE 8080
ENTRYPOINT ["/smoke"]
