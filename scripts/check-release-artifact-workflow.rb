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
  "runs-on: ${{ matrix.runner }}",
  "runner: ubuntu-24.04",
  "runner: ubuntu-24.04-arm",
  "target: x86_64-unknown-linux-gnu",
  "target: aarch64-unknown-linux-gnu",
  "platform: linux/amd64",
  "platform: linux/arm64",
  "base_image: debian:bookworm-slim@sha256:63a496b5d3b99214b39f5ed70eb71a61e590a77979c79cbee4faf991f8c0783e",
  "base_image: debian:bookworm-slim@sha256:9b67294679b30e5d6ab257b40594feeb4a4b81f7fcf4131f4decf0d6a212a9b0",
  "Verify the runner architecture matches the release target",
  'actual_machine="$(uname -m)"',
  "cargo +1.97.1 build --locked --release --package gateway",
  "generate-p12-01-sbom.rb",
  "p12-release-artifact.rb manifest",
  "p12-release-artifact.rb verify",
  "docker buildx create",
  "--driver docker-container",
  "docker buildx inspect --bootstrap",
  "docker buildx build",
  '--builder "$builder_name"',
  '--platform "$RELEASE_PLATFORM"',
  '--build-arg RELEASE_BASE_IMAGE="$RELEASE_BASE_IMAGE"',
  "type=oci,dest=release-artifact/gateway-image.oci.tar",
  'type=docker,dest=$DOCKER_ARCHIVE',
  'docker load --input "$DOCKER_ARCHIVE"',
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

# The release targets must be native-built: a target may only run on a runner of its own
# architecture, and no emulation layer may be introduced. The base image digests must match the
# verifier's closed target table, since the Dockerfile now receives the base image as an argument.
verifier_path = File.join(root, "scripts", "p12-release-artifact.rb")
verifier_text = File.read(verifier_path, encoding: "UTF-8")
EXPECTED_MATRIX = {
  "x86_64-unknown-linux-gnu" => { runner: "ubuntu-24.04", platform: "linux/amd64" },
  "aarch64-unknown-linux-gnu" => { runner: "ubuntu-24.04-arm", platform: "linux/arm64" }
}.freeze

begin
  document = Psych.load_file(path)
  job = document.fetch("jobs").fetch("build")
  errors << "release-artifact build job must run on the matrix runner" unless job["runs-on"] == "${{ matrix.runner }}"
  entries = job.dig("strategy", "matrix", "include")
  if entries.is_a?(Array)
    observed = entries.to_h do |entry|
      [entry["target"], { runner: entry["runner"], platform: entry["platform"] }]
    end
    unless observed == EXPECTED_MATRIX
      errors << "release-artifact matrix must natively build exactly #{EXPECTED_MATRIX.keys.join(" and ")}"
    end
    entries.each do |entry|
      base = entry["base_image"].to_s
      unless base.match?(/\Adebian:bookworm-slim@sha256:[0-9a-f]{64}\z/)
        errors << "release-artifact base image for #{entry["target"]} must be a digest-pinned debian:bookworm-slim"
        next
      end
      unless verifier_text.include?("base_image: \"#{base}\"")
        errors << "release-artifact base image for #{entry["target"]} is not the digest the verifier enforces"
      end
    end
    digests = entries.map { |entry| entry["base_image"] }
    errors << "each release target must pin its own base image digest" unless digests.uniq.length == digests.length
  else
    errors << "release-artifact must declare a build matrix with an include list"
  end
rescue Psych::SyntaxError, KeyError, TypeError => error
  errors << "unable to inspect release-artifact structure: #{error.message}"
end

# The base image reaches the Dockerfile as a build argument, and an ARG referenced by a FROM is
# stage-scoped unless it is declared before the first FROM. Getting this wrong makes the base name
# resolve to the empty string, which only surfaces as a build failure in CI.
dockerfile_path = File.join(root, "Dockerfile")
begin
  dockerfile_lines = File.read(dockerfile_path, encoding: "UTF-8").lines
  global_args = []
  seen_from = false
  from_args = []
  dockerfile_lines.each do |line|
    directive = line.strip
    next if directive.empty? || directive.start_with?("#")

    if (match = directive.match(/\AARG\s+([A-Za-z_][A-Za-z0-9_]*)/i))
      global_args << match[1] unless seen_from
    elsif directive.match?(/\AFROM\s/i)
      from_args.concat(directive.scan(/\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?/).flatten)
      seen_from = true
    end
  end
  from_args.uniq.each do |name|
    unless global_args.include?(name)
      errors << "Dockerfile ARG #{name} is referenced by FROM but not declared before the first FROM"
    end
  end
  unless from_args.include?("RELEASE_BASE_IMAGE")
    errors << "Dockerfile must take its base image from the RELEASE_BASE_IMAGE build argument"
  end
  unless dockerfile_lines.any? { |line| line.include?('org.opencontainers.image.base.name="${RELEASE_BASE_IMAGE}"') }
    errors << "Dockerfile must label the image with the base image it was actually built from"
  end
rescue Errno::ENOENT
  errors << "Dockerfile is missing"
end

directive_lines = text.lines.reject { |line| line.strip.start_with?("#") }.join
%w[setup-qemu-action qemu binfmt --platform\ linux/amd64,linux/arm64].each do |forbidden|
  if directive_lines.match?(/#{Regexp.escape(forbidden)}/i)
    errors << "release-artifact must build natively, not under emulation: #{forbidden}"
  end
end

if text.include?("docker load --input release-artifact/gateway-image.oci.tar")
  errors << "release-artifact must not pass an OCI layout archive to docker load"
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
  puts "release-artifact-workflow: ok (manual, pinned, native dual-target, private and keyless)"
else
  warn errors.join("\n")
  exit 1
end
