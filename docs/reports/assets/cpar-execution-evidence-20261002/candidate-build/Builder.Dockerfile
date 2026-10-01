FROM node:24.19.0-bookworm-slim@sha256:a9f5f7c91a432850b2a8a7797adf5eadb6c733ceed61167806cee7ea7fbc29df AS node
FROM rust:1.97.1-bookworm@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
COPY --from=node /usr/local/ /usr/local/
RUN apt-get update && apt-get install --no-install-recommends --yes clang libclang-dev cmake ruby ripgrep python3 file && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
