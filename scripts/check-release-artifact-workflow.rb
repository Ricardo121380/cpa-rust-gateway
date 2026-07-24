#!/usr/bin/env ruby
# frozen_string_literal: true

require "psych"

root = File.expand_path("..", __dir__)
path = File.join(root, ".github", "workflows", "release-artifact.yml")
text = File.read(path, encoding: "UTF-8")
errors = []

begin
  Psych.parse(text)
rescue Psych::SyntaxError => error
  errors << "invalid YAML: #{error.message}"
end

required_fragments = [
  "name: release-artifact",
  "workflow_dispatch:",
  "contents: read",
  "id-token: write",
  "runs-on: ubuntu-24.04",
  "x86_64-unknown-linux-gnu",
  "cargo +1.97.1 build --locked --release --package gateway",
  "generate-p12-01-sbom.rb",
  "p12-release-artifact.rb manifest",
  "p12-release-artifact.rb verify",
  "docker buildx build",
  "type=oci,dest=release-artifact/gateway-image.oci.tar",
  "docker load --input release-artifact/gateway-image.oci.tar",
  "--network none --read-only --user 65532:65532",
  "cosign sign-blob --yes",
  "cosign verify-blob",
  "--certificate-identity",
  "--certificate-oidc-issuer",
  "p12-release-artifact.rb receipt",
  "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
  "retention-days: 14"
]
required_fragments.each do |fragment|
  errors << "missing required release-artifact workflow fragment: #{fragment}" unless text.include?(fragment)
end

expected_trigger = <<~YAML
  on:
    workflow_dispatch:
YAML
errors << "release-artifact must be manual-dispatch only" unless text.include?(expected_trigger)

text.scan(/^\s*-\s+uses:\s*([^\s#]+)/).flatten.each do |action|
  reference = action.split("@", 2)[1]
  unless reference && reference.match?(/\A[0-9a-f]{40}\z/)
    errors << "GitHub Action is not pinned to a full commit SHA: #{action}"
  end
end

%w[docker\ push buildx\ imagetools\ create gh\ release git\ tag registry\.].each do |forbidden|
  errors << "release-artifact contains forbidden publish/deploy command: #{forbidden}" if text.match?(/#{forbidden}/i)
end

if errors.empty?
  puts "release-artifact-workflow: ok (manual, pinned, private and keyless)"
else
  warn errors.join("\n")
  exit 1
end
