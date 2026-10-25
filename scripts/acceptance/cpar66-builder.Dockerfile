# Dependency preparation only. Product compilation is a separate --network=none run.
FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
RUN apt-get update \
    && apt-get install --yes --no-install-recommends clang cmake libclang-dev ruby python3 iproute2 iptables \
    && mkdir /builder-provenance \
    && dpkg-query -W > /builder-provenance/packages.tsv \
    && sha256sum /var/lib/apt/lists/*InRelease > /builder-provenance/inrelease-sha256.txt \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install --locked cargo-cyclonedx --version 0.5.9
RUN rustup component add clippy rustfmt --toolchain 1.97.1
