# Dependency preparation only. Product compilation is a separate --network=none run.
FROM rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
RUN apt-get update \
    && apt-get install --yes --no-install-recommends clang cmake libclang-dev ruby python3 iproute2 iptables \
    && mkdir /builder-provenance \
    && dpkg-query -W > /builder-provenance/packages.tsv \
    && sha256sum /var/lib/apt/lists/*InRelease > /builder-provenance/inrelease-sha256.txt \
    && rm -rf /var/lib/apt/lists/*
RUN curl --fail --silent --show-error --location https://nodejs.org/dist/v24.19.0/node-v24.19.0-linux-arm64.tar.xz --output /tmp/node.tar.xz \
    && echo '01443c1e1a29e531ccad5a46fefa6df490d2189c49f7955904aecdbb0fe86fdc  /tmp/node.tar.xz' | sha256sum --check \
    && tar -xJf /tmp/node.tar.xz -C /usr/local --strip-components=1 \
    && rm /tmp/node.tar.xz \
    && test "$(node --version)" = v24.19.0 \
    && test "$(rustc --version | cut -d ' ' -f 2)" = 1.97.1
RUN cargo install --locked cargo-cyclonedx --version 0.5.9
RUN rustup component add clippy rustfmt --toolchain 1.97.1
