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
TARGET = "x86_64-unknown-linux-gnu"
VERSION = "0.1.0"

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

def write_oci(path, root_user: "65532:65532", unsafe_path: false)
  layer = "p12 test layer"
  config = JSON.generate(
    "architecture" => "amd64",
    "os" => "linux",
    "config" => {
      "User" => root_user,
      "Entrypoint" => ["/usr/local/bin/gateway"],
      "Labels" => {
        "org.opencontainers.image.revision" => REVISION,
        "org.opencontainers.image.version" => VERSION,
        "org.opencontainers.image.rust.toolchain" => TOOLCHAIN,
        "org.opencontainers.image.target" => TARGET
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
      write_tar_entry(writer, "oci-layout", JSON.generate("imageLayoutVersion" => "1.0.0"))
      write_tar_entry(writer, "index.json", index)
      write_tar_entry(writer, "blobs/sha256/#{Digest::SHA256.hexdigest(config)}", config)
      write_tar_entry(writer, "blobs/sha256/#{Digest::SHA256.hexdigest(manifest)}", manifest)
      write_tar_entry(writer, "blobs/sha256/#{Digest::SHA256.hexdigest(layer)}", layer)
      write_tar_entry(writer, "../outside", "unsafe") if unsafe_path
    end
  end
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

def common_arguments(directory)
  [
    "--artifact-dir", directory,
    "--revision", REVISION,
    "--rust-toolchain", TOOLCHAIN,
    "--target", TARGET,
    "--version", VERSION
  ]
end

Dir.mktmpdir("p12-artifact-test") do |temporary|
  artifact_dir = File.join(temporary, "artifact")
  FileUtils.mkdir_p(artifact_dir)
  elf_header = "\x7fELF\x02\x01\x01\x00\x00" + ("\x00" * 7) + [2, 62].pack("v2")
  File.binwrite(
    File.join(artifact_dir, "gateway-x86_64-unknown-linux-gnu"),
    elf_header + "gateway-release-revision=#{REVISION}\n" \
    "gateway-release-rust-version=#{TOOLCHAIN}\n" \
    "gateway-release-target=#{TARGET}\n"
  )
  run_tool(
    "build-metadata",
    "--revision", REVISION,
    "--rust-toolchain", TOOLCHAIN,
    "--target", TARGET,
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
        "properties" => [{ "name" => "cdx:rustc:sbom:target:triple", "value" => TARGET }]
      },
      "components" => [{ "name" => "gateway", "version" => VERSION, "purl" => "pkg:cargo/gateway@#{VERSION}" }]
    )
  )
  write_oci(File.join(artifact_dir, "gateway-image.oci.tar"))

  arguments = common_arguments(artifact_dir)
  run_tool("manifest", *arguments, "--output", File.join(artifact_dir, "artifact-manifest.json"))
  run_tool("verify", *arguments)

  binary_path = File.join(artifact_dir, "gateway-x86_64-unknown-linux-gnu")
  original_binary = File.binread(binary_path)
  wrong_architecture = original_binary.dup
  wrong_architecture[18, 2] = [183].pack("v")
  File.binwrite(binary_path, wrong_architecture)
  reject_tool("verify", *arguments)
  File.binwrite(binary_path, original_binary)
  run_tool("verify", *arguments)

  File.write(File.join(artifact_dir, "unexpected.txt"), "unexpected")
  reject_tool("verify", *arguments)
  FileUtils.rm(File.join(artifact_dir, "unexpected.txt"))

  manifest_path = File.join(artifact_dir, "artifact-manifest.json")
  manifest = JSON.parse(File.read(manifest_path))
  manifest.fetch("files").last["sha256"] = "0" * 64
  File.write(manifest_path, JSON.generate(manifest))
  reject_tool("verify", *arguments)
  run_tool("manifest", *arguments, "--output", manifest_path)

  write_oci(File.join(artifact_dir, "gateway-image.oci.tar"), root_user: "0")
  reject_tool("verify", *arguments)
  write_oci(File.join(artifact_dir, "gateway-image.oci.tar"))
  run_tool("manifest", *arguments, "--output", manifest_path)
  run_tool("verify", *arguments)

  write_oci(File.join(artifact_dir, "gateway-image.oci.tar"), unsafe_path: true)
  reject_tool("verify", *arguments)
  write_oci(File.join(artifact_dir, "gateway-image.oci.tar"))
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
end

puts "p12-artifact-test: manifest, receipt, SBOM and OCI rejection paths passed"
