#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "fileutils"
require "json"
require "open3"
require "rbconfig"
require "rubygems/package"
require "stringio"
require "tmpdir"

ROOT = File.expand_path("..", __dir__)
SCRIPT = File.join(ROOT, "scripts", "p12-release-artifact.rb")
REVISION = "a" * 40
TOOLCHAIN = "1.97.1"
VERSION = "0.1.0"
# Mirrors the script's closed target table. Each target is verified independently, and the
# cross-architecture cases below assert that one target never accepts the other's payloads.
TARGETS = {
  "x86_64-unknown-linux-gnu" => {
    elf_machine: 62,
    oci_architecture: "amd64",
    base_image: "debian:bookworm-slim@sha256:63a496b5d3b99214b39f5ed70eb71a61e590a77979c79cbee4faf991f8c0783e"
  },
  "aarch64-unknown-linux-gnu" => {
    elf_machine: 183,
    oci_architecture: "arm64",
    base_image: "debian:bookworm-slim@sha256:9b67294679b30e5d6ab257b40594feeb4a4b81f7fcf4131f4decf0d6a212a9b0"
  }
}.freeze

def digest_descriptor(bytes, media_type)
  {
    "mediaType" => media_type,
    "digest" => "sha256:#{Digest::SHA256.hexdigest(bytes)}",
    "size" => bytes.bytesize
  }
end

def write_tar_entry(writer, name, bytes)
  writer.add_file_simple(name, 0o644, bytes.bytesize) { |io| io.write(bytes) }
end

def write_oci(path, target, root_user: "65532:65532", unsafe_path: false, unsafe_symlink: false,
              architecture: nil, base_image: nil)
  layer = "p12 test layer"
  config = JSON.generate(
    "architecture" => architecture || TARGETS.fetch(target).fetch(:oci_architecture),
    "os" => "linux",
    "config" => {
      "User" => root_user,
      "Entrypoint" => ["/usr/local/bin/gateway"],
      "Labels" => {
        "org.opencontainers.image.revision" => REVISION,
        "org.opencontainers.image.version" => VERSION,
        "org.opencontainers.image.rust.toolchain" => TOOLCHAIN,
        "org.opencontainers.image.target" => target,
        "org.opencontainers.image.base.name" => base_image || TARGETS.fetch(target).fetch(:base_image)
      }
    }
  )
  manifest = JSON.generate(
    "schemaVersion" => 2,
    "config" => digest_descriptor(config, "application/vnd.oci.image.config.v1+json"),
    "layers" => [digest_descriptor(layer, "application/vnd.oci.image.layer.v1.tar")]
  )
  index = JSON.generate(
    "schemaVersion" => 2,
    "manifests" => [digest_descriptor(manifest, "application/vnd.oci.image.manifest.v1+json")]
  )
  File.open(path, "wb") do |file|
    Gem::Package::TarWriter.new(file) do |writer|
      writer.mkdir("blobs/", 0o755)
      writer.mkdir("blobs/sha256/", 0o755)
      write_tar_entry(writer, "oci-layout", JSON.generate("imageLayoutVersion" => "1.0.0"))
      write_tar_entry(writer, "index.json", index)
      write_tar_entry(writer, "blobs/sha256/#{Digest::SHA256.hexdigest(config)}", config)
      write_tar_entry(writer, "blobs/sha256/#{Digest::SHA256.hexdigest(manifest)}", manifest)
      write_tar_entry(writer, "blobs/sha256/#{Digest::SHA256.hexdigest(layer)}", layer)
      write_tar_entry(writer, "../outside", "unsafe") if unsafe_path
      writer.add_symlink("blobs/sha256/link", "../outside", 0o777) if unsafe_symlink
    end
  end
end

def write_binary(path, target, elf_machine: nil)
  machine = elf_machine || TARGETS.fetch(target).fetch(:elf_machine)
  header = "\x7fELF\x02\x01\x01\x00\x00" + ("\x00" * 7) + [2, machine].pack("v2")
  File.binwrite(
    path,
    header + "gateway-release-revision=#{REVISION}\n" \
    "gateway-release-rust-version=#{TOOLCHAIN}\n" \
    "gateway-release-target=#{target}\n"
  )
end

def run_tool(*arguments)
  stdout, stderr, status = Open3.capture3(RbConfig.ruby, SCRIPT, *arguments)
  raise "command failed: #{arguments.join(" ")}\n#{stdout}\n#{stderr}" unless status.success?

  stdout
end

def reject_tool(*arguments)
  _stdout, _stderr, status = Open3.capture3(RbConfig.ruby, SCRIPT, *arguments)
  raise "command unexpectedly passed: #{arguments.join(" ")}" if status.success?
end

def common_arguments(directory, target)
  [
    "--artifact-dir", directory,
    "--revision", REVISION,
    "--rust-toolchain", TOOLCHAIN,
    "--target", target,
    "--version", VERSION
  ]
end

def exercise_target(target)
  Dir.mktmpdir("p12-artifact-test") do |temporary|
    artifact_dir = File.join(temporary, "artifact")
    FileUtils.mkdir_p(artifact_dir)
    binary_path = File.join(artifact_dir, "gateway-#{target}")
    write_binary(binary_path, target)
    run_tool(
      "build-metadata",
      "--revision", REVISION,
      "--rust-toolchain", TOOLCHAIN,
      "--target", target,
      "--version", VERSION,
      "--output", File.join(artifact_dir, "gateway-build-metadata.json")
    )
    run_tool(
      "identity",
      "--repository", "Ricardo121380/cpa-rust-gateway",
      "--workflow-path", ".github/workflows/release-artifact.yml",
      "--ref", "refs/heads/codex/p11-release-hardening",
      "--output", File.join(artifact_dir, "signing-identity.json")
    )
    File.write(
      File.join(artifact_dir, "gateway-sbom.cdx.json"),
      JSON.generate(
        "bomFormat" => "CycloneDX",
        "specVersion" => "1.5",
        "metadata" => {
          "properties" => [{ "name" => "cdx:rustc:sbom:target:triple", "value" => target }]
        },
        "components" => [{ "name" => "gateway", "version" => VERSION, "purl" => "pkg:cargo/gateway@#{VERSION}" }]
      )
    )
    oci_path = File.join(artifact_dir, "gateway-image.oci.tar")
    write_oci(oci_path, target)

    arguments = common_arguments(artifact_dir, target)
    manifest_path = File.join(artifact_dir, "artifact-manifest.json")
    run_tool("manifest", *arguments, "--output", manifest_path)
    run_tool("verify", *arguments)

    # The other target's ELF machine must be rejected even though every other payload matches.
    other_target = (TARGETS.keys - [target]).fetch(0)
    write_binary(binary_path, target, elf_machine: TARGETS.fetch(other_target).fetch(:elf_machine))
    reject_tool("verify", *arguments)
    write_binary(binary_path, target, elf_machine: 40)
    reject_tool("verify", *arguments)
    write_binary(binary_path, target)
    run_tool("verify", *arguments)

    # The other target's OCI architecture must be rejected for the same reason.
    write_oci(oci_path, target, architecture: TARGETS.fetch(other_target).fetch(:oci_architecture))
    reject_tool("verify", *arguments)
    write_oci(oci_path, target)
    run_tool("manifest", *arguments, "--output", manifest_path)
    run_tool("verify", *arguments)

    # Now that the base image arrives as a build argument, an unpinned or wrong-architecture base
    # must be rejected from the produced image itself.
    write_oci(oci_path, target, base_image: TARGETS.fetch(other_target).fetch(:base_image))
    reject_tool("verify", *arguments)
    write_oci(oci_path, target, base_image: "debian:bookworm-slim")
    reject_tool("verify", *arguments)
    write_oci(oci_path, target)
    run_tool("manifest", *arguments, "--output", manifest_path)
    run_tool("verify", *arguments)

    # A payload set named for the other target cannot satisfy this target.
    FileUtils.mv(binary_path, File.join(artifact_dir, "gateway-#{other_target}"))
    reject_tool("verify", *arguments)
    FileUtils.mv(File.join(artifact_dir, "gateway-#{other_target}"), binary_path)
    run_tool("verify", *arguments)

    File.write(File.join(artifact_dir, "unexpected.txt"), "unexpected")
    reject_tool("verify", *arguments)
    FileUtils.rm(File.join(artifact_dir, "unexpected.txt"))

    manifest = JSON.parse(File.read(manifest_path))
    manifest.fetch("files").last["sha256"] = "0" * 64
    File.write(manifest_path, JSON.generate(manifest))
    reject_tool("verify", *arguments)
    run_tool("manifest", *arguments, "--output", manifest_path)

    write_oci(oci_path, target, root_user: "0")
    reject_tool("verify", *arguments)
    write_oci(oci_path, target)
    run_tool("manifest", *arguments, "--output", manifest_path)
    run_tool("verify", *arguments)

    write_oci(oci_path, target, unsafe_path: true)
    reject_tool("verify", *arguments)
    write_oci(oci_path, target)
    run_tool("manifest", *arguments, "--output", manifest_path)
    run_tool("verify", *arguments)

    write_oci(oci_path, target, unsafe_symlink: true)
    reject_tool("verify", *arguments)
    write_oci(oci_path, target)
    run_tool("manifest", *arguments, "--output", manifest_path)
    run_tool("verify", *arguments)

    File.write(File.join(artifact_dir, "artifact-manifest.json.sig"), "test-signature")
    File.write(File.join(artifact_dir, "artifact-manifest.sigstore.json"), "test-bundle")
    run_tool(
      "receipt", *arguments,
      "--workflow-run", "https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/123",
      "--output", File.join(artifact_dir, "artifact-receipt.json")
    )
    run_tool("verify", *arguments, "--require-signature", "--require-receipt")

    # A verified artifact set still fails when checked against the other target.
    reject_tool(
      "verify", *common_arguments(artifact_dir, other_target),
      "--require-signature", "--require-receipt"
    )
  end
end

TARGETS.each_key { |target| exercise_target(target) }

# Targets outside the closed table fail closed in every command that accepts one.
Dir.mktmpdir("p12-artifact-target") do |temporary|
  reject_tool(
    "build-metadata",
    "--revision", REVISION,
    "--rust-toolchain", TOOLCHAIN,
    "--target", "riscv64gc-unknown-linux-gnu",
    "--version", VERSION,
    "--output", File.join(temporary, "gateway-build-metadata.json")
  )
  reject_tool(*common_arguments(temporary, "riscv64gc-unknown-linux-gnu"))
  reject_tool("verify", *common_arguments(temporary, "x86_64-apple-darwin"))
end

puts "p12-artifact-test: dual-target manifest, receipt, SBOM, OCI and cross-architecture rejection paths passed"
