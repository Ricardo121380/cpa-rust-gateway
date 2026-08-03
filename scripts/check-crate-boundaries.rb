#!/usr/bin/env ruby

require "json"
require "English"

root = File.expand_path("..", __dir__)
metadata_json = IO.popen(["cargo", "metadata", "--format-version", "1", "--no-deps"], chdir: root, &:read)
abort "crate-boundaries: cargo metadata failed" unless $CHILD_STATUS.success?

metadata = JSON.parse(metadata_json)
workspace_ids = metadata.fetch("workspace_members")
packages = metadata.fetch("packages").select { |package| workspace_ids.include?(package.fetch("id")) }
workspace_names = packages.map { |package| package.fetch("name") }.sort

allowed = {
  # The binary is the sole deployment-composition root. P12-05 adds the explicitly bounded
  # data-plane adapter and its immutable RouteSnapshot, encrypted Credential-pool, direct egress,
  # protocol, and JSON dependencies here only; lower-layer rules still prevent them from flowing
  # back into library crates.
  "gateway" => %w[actix-web futures-util gateway-auth gateway-catalog gateway-control gateway-core gateway-http-actix gateway-observability gateway-protocol gateway-router gateway-store gateway-upstream libc protocol-openai-chat protocol-openai-responses provider-anthropic-compatible provider-grok provider-kiro provider-openai-compatible serde_json zeroize],
  "gateway-auth" => %w[gateway-core getrandom hmac libc sha2 subtle zeroize],
  "gateway-catalog" => %w[gateway-core gateway-provider tokio],
  "gateway-control" => %w[gateway-auth gateway-catalog gateway-core gateway-observability gateway-protocol gateway-router gateway-store gateway-upstream serde_json zeroize],
  "gateway-core" => %w[serde serde_json],
  # P11-01/P12-08D4's differential gate is a test-only leaf under tests/differential. It may depend
  # on the protocol/router crates whose behavior it projects because nothing depends on it; the
  # direction never reverses.
  "differential-gate" => %w[gateway-core gateway-router gateway-store gateway-upstream protocol-anthropic protocol-openai-chat protocol-openai-responses provider-grok provider-kiro serde serde_json],
  "gateway-http-actix" => %w[actix-web criterion futures-util gateway-auth gateway-control gateway-core gateway-observability gateway-protocol gateway-router gateway-store gateway-stream gateway-upstream getrandom protocol-anthropic protocol-openai-chat protocol-openai-responses provider-openai-compatible serde serde_json subtle tokio url zeroize],
  "gateway-observability" => %w[gateway-core opentelemetry serde serde_json sha2 tokio tracing tracing-subscriber],
  "gateway-protocol" => %w[gateway-core],
  "gateway-provider" => %w[criterion gateway-core serde serde_json tokio],
  # Protocol/provider dependencies below are dev-only D1-D3 matrix tests; production routing still
  # depends only on transport-neutral gateway interfaces.
  "gateway-router" => %w[arc-swap gateway-auth gateway-catalog gateway-core gateway-protocol gateway-provider gateway-upstream proptest protocol-anthropic protocol-openai-chat protocol-openai-responses provider-anthropic-compatible provider-openai-compatible serde_json tokio],
  "gateway-store" => %w[chacha20poly1305 gateway-core gateway-observability getrandom libc rusqlite serde_json sha2 tokio zeroize],
  "gateway-stream" => %w[gateway-core gateway-protocol proptest tokio tokio-util],
  "gateway-upstream" => %w[bytes gateway-auth gateway-core gateway-provider moka reqwest tokio url zeroize],
  "protocol-anthropic" => %w[gateway-core gateway-protocol proptest serde serde_json],
  "protocol-openai-chat" => %w[gateway-core proptest serde serde_json],
  "protocol-openai-responses" => %w[gateway-core gateway-protocol proptest serde serde_json],
  "provider-anthropic-compatible" => %w[gateway-core gateway-provider gateway-upstream protocol-anthropic serde serde_json zeroize],
  "provider-grok" => %w[flate2 gateway-catalog gateway-core gateway-provider gateway-router gateway-store gateway-upstream getrandom hmac protocol-openai-responses rusqlite serde serde_json sha2 time tokio url zeroize],
  "provider-kiro" => %w[gateway-catalog gateway-core gateway-provider gateway-store gateway-stream gateway-upstream protocol-anthropic serde serde_json tokio url zeroize],
  "provider-openai-compatible" => %w[gateway-core gateway-provider gateway-upstream protocol-openai-chat protocol-openai-responses serde serde_json zeroize],
}

errors = []
errors << "workspace member set differs from boundary policy" unless workspace_names == allowed.keys.sort

packages.each do |package|
  name = package.fetch("name")
  actual = package.fetch("dependencies").map { |dependency| dependency.fetch("name") }.uniq.sort
  expected = allowed.fetch(name, []).sort
  errors << "#{name}: expected #{expected.inspect}, got #{actual.inspect}" unless actual == expected
end

if errors.empty?
  puts "crate-boundaries: ok (#{packages.length} workspace packages)"
else
  warn errors.join("\n")
  exit 1
end
