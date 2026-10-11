# Dependency preparation has network access; the subsequent product build does not.
FROM cpar-build-candidate@sha256:436d5811a29917a624dd155d49501f6edb19d2ab62a1dcd308555923dea4b8e7
USER root
RUN mkdir -p /builder-provenance/debs/partial \
    && cp /etc/apt/sources.list.d/debian.sources /builder-provenance/debian.sources \
    && dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\n' > /builder-provenance/packages-before.tsv \
    && apt-get update \
    && apt-get --print-uris --yes --no-install-recommends install ruby > /builder-provenance/package-uris.txt \
    && apt-get --yes --no-install-recommends -o Dir::Cache::archives=/builder-provenance/debs install ruby \
    && dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\n' > /builder-provenance/packages-after.tsv \
    && sha256sum /builder-provenance/debs/*.deb > /builder-provenance/deb-sha256.txt \
    && sha256sum /var/lib/apt/lists/*InRelease > /builder-provenance/inrelease-sha256.txt \
    && ruby --version > /builder-provenance/ruby-version.txt \
    && ruby -rdigest -ropen3 -rjson -e 'abort unless Digest::SHA256.hexdigest("cpar").size == 64; abort unless Open3.capture3("true")[2].success?; puts JSON.generate({stdlib: "PASS"})' > /builder-provenance/stdlib.json
