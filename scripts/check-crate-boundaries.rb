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
  "gateway" => %w[gateway-control gateway-http-actix gateway-observability],
  "gateway-access" => %w[gateway-core],
  "gateway-auth" => %w[gateway-core getrandom hmac libc sha2 subtle zeroize],
  "gateway-catalog" => %w[gateway-core gateway-provider],
  "gateway-continuity" => %w[gateway-core],
  "gateway-control" => %w[gateway-access gateway-auth gateway-catalog gateway-core gateway-observability gateway-router gateway-store gateway-upstream serde_json],
  "gateway-core" => %w[serde serde_json],
  "gateway-http-actix" => %w[actix-web futures-util gateway-auth gateway-control gateway-core gateway-observability gateway-protocol gateway-router gateway-stream protocol-anthropic protocol-openai-responses tokio],
  "gateway-observability" => %w[gateway-core],
  "gateway-protocol" => %w[gateway-core],
  "gateway-provider" => %w[gateway-core serde serde_json tokio],
  "gateway-router" => %w[arc-swap gateway-access gateway-auth gateway-catalog gateway-continuity gateway-core gateway-provider gateway-upstream tokio],
  "gateway-store" => %w[chacha20poly1305 gateway-core getrandom libc rusqlite zeroize],
  "gateway-stream" => %w[gateway-core gateway-protocol tokio tokio-util],
  "gateway-upstream" => %w[bytes gateway-auth gateway-core gateway-provider moka reqwest tokio url zeroize],
  "protocol-anthropic" => %w[gateway-core gateway-protocol],
  "protocol-openai-responses" => %w[gateway-core gateway-protocol proptest serde serde_json],
  "provider-anthropic-compatible" => %w[gateway-core gateway-provider gateway-upstream protocol-anthropic],
  "provider-grok" => %w[gateway-continuity gateway-core gateway-provider gateway-upstream protocol-openai-responses],
  "provider-kiro" => %w[gateway-core gateway-provider gateway-stream gateway-upstream protocol-anthropic],
  "provider-openai-compatible" => %w[gateway-core gateway-provider gateway-upstream protocol-openai-responses serde_json zeroize],
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
