#!/usr/bin/env ruby
# frozen_string_literal: true

# P12's artifact format is deliberately small and fail-closed. A manifest covers the immutable
# payloads; the Sigstore signature covers that manifest; a receipt records the verified signing
# result without containing a key, credential, provider value or server address.

require "digest"
require "json"
require "optparse"
require "rubygems/package"

module P12ReleaseArtifact
  MANIFEST_NAME = "artifact-manifest.json"
  SIGNATURE_NAME = "artifact-manifest.json.sig"
  BUNDLE_NAME = "artifact-manifest.sigstore.json"
  RECEIPT_NAME = "artifact-receipt.json"
  PAYLOAD_NAMES = [
    "gateway-build-metadata.json",
    "gateway-image.oci.tar",
    "gateway-sbom.cdx.json",
    "gateway-x86_64-unknown-linux-gnu",
    "signing-identity.json"
  ].freeze
  MANIFEST_SCHEMA = "cpa-rust-gateway-artifact-manifest-v1"
  BUILD_METADATA_SCHEMA = "cpa-rust-gateway-build-metadata-v1"
  SIGNING_IDENTITY_SCHEMA = "cpa-rust-gateway-signing-identity-v1"
  RECEIPT_SCHEMA = "cpa-rust-gateway-artifact-receipt-v1"
  TARGET = "x86_64-unknown-linux-gnu"
  OIDC_ISSUER = "https://token.actions.githubusercontent.com"
  LOCAL_REFERENCE = %r{(?:path\+file:|file://|https?://|/Users/|/home/|[A-Za-z]:\\\\Users\\)}i
  FORBIDDEN_SBOM_KEYS = %w[
    authorization cookie credential endpoint password secret token url
  ].freeze

  class Failure < StandardError; end

  module_function

  def fail!(message)
    raise Failure, message
  end

  def parse_options(command)
    values = {}
    parser = OptionParser.new
    parser.banner = "usage: #{$PROGRAM_NAME} #{command} [options]"
    yield parser, values
    parser.parse!
    fail!("#{command}: unexpected arguments") unless ARGV.empty?
    values
  rescue OptionParser::ParseError => error
    fail!("#{command}: #{error.message}")
  end

  def required!(values, key, command)
    value = values[key]
    fail!("#{command}: --#{key.to_s.tr("_", "-")} is required") if value.to_s.empty?

    value
  end

  def read_json(path, label)
    JSON.parse(File.read(path, encoding: "UTF-8"))
  rescue Errno::ENOENT
    fail!("#{label} is missing: #{path}")
  rescue JSON::ParserError => error
    fail!("#{label} is not JSON: #{error.message}")
  end

  def write_json(path, object)
    File.write(path, "#{JSON.pretty_generate(object)}\n")
  end

  def sha256(path)
    Digest::SHA256.file(path).hexdigest
  end

  def sha256_text(text)
    Digest::SHA256.hexdigest(text)
  end

  def valid_revision?(value)
    value.is_a?(String) && value.match?(/\A[0-9a-f]{40}\z/)
  end

  def valid_version?(value)
    value.is_a?(String) && value.match?(/\A[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?\z/)
  end

  def valid_toolchain?(value)
    value.is_a?(String) && value.match?(/\A[0-9]+\.[0-9]+\.[0-9]+\z/)
  end

  def require_exact_keys!(value, expected, label)
    fail!("#{label} must be an object") unless value.is_a?(Hash)
    keys = value.keys.sort
    fail!("#{label} has unexpected or missing keys") unless keys == expected.sort
  end

  def require_regular_file!(path, label)
    stat = File.lstat(path)
    fail!("#{label} must be a regular file") unless stat.file? && !stat.symlink?
  rescue Errno::ENOENT
    fail!("#{label} is missing")
  end

  def directory_entries(directory)
    fail!("artifact directory is missing: #{directory}") unless Dir.exist?(directory)
    entries = Dir.children(directory).sort
    entries.each do |name|
      fail!("artifact entry has an invalid name: #{name.inspect}") unless name.match?(/\A[A-Za-z0-9][A-Za-z0-9._-]*\z/)
      require_regular_file!(File.join(directory, name), "artifact entry #{name}")
    end
    entries
  end

  def release_options(command, require_output: false)
    values = parse_options(command) do |parser, result|
      parser.on("--artifact-dir DIRECTORY", String) { |value| result[:artifact_dir] = value }
      parser.on("--revision SHA", String) { |value| result[:revision] = value }
      parser.on("--rust-toolchain VERSION", String) { |value| result[:rust_toolchain] = value }
      parser.on("--target TARGET", String) { |value| result[:target] = value }
      parser.on("--version VERSION", String) { |value| result[:version] = value }
      parser.on("--output PATH", String) { |value| result[:output] = value } if require_output
      yield parser, result if block_given?
    end
    %i[artifact_dir revision rust_toolchain target version].each { |key| required!(values, key, command) }
    required!(values, :output, command) if require_output
    fail!("#{command}: revision must be a full lowercase Git SHA") unless valid_revision?(values[:revision])
    fail!("#{command}: invalid Rust toolchain") unless valid_toolchain?(values[:rust_toolchain])
    fail!("#{command}: target must be #{TARGET}") unless values[:target] == TARGET
    fail!("#{command}: invalid release version") unless valid_version?(values[:version])
    values
  end

  def validate_build_metadata!(directory, values)
    path = File.join(directory, "gateway-build-metadata.json")
    build = read_json(path, "build metadata")
    require_exact_keys!(build, %w[binary revision rust_toolchain schema_version target version], "build metadata")
    fail!("build metadata schema mismatch") unless build["schema_version"] == BUILD_METADATA_SCHEMA
    fail!("build metadata binary mismatch") unless build["binary"] == "gateway-x86_64-unknown-linux-gnu"
    %i[revision rust_toolchain target version].each do |key|
      fail!("build metadata #{key} mismatch") unless build[key.to_s] == values[key]
    end
  end

  def validate_binary_metadata!(directory, values)
    path = File.join(directory, "gateway-x86_64-unknown-linux-gnu")
    bytes = File.binread(path)
    elf_identity = "\x7fELF\x02\x01"
    fail!("release binary is not a 64-bit little-endian ELF executable") unless bytes.start_with?(elf_identity)
    fail!("release binary is not x86_64") unless bytes.byteslice(18, 2) == [62].pack("v")
    {
      "gateway-release-revision" => values[:revision],
      "gateway-release-rust-version" => values[:rust_toolchain],
      "gateway-release-target" => values[:target]
    }.each do |key, value|
      fail!("release binary does not embed #{key}") unless bytes.include?("#{key}=#{value}\n")
    end
  end

  def validate_signing_identity!(directory)
    identity = read_json(File.join(directory, "signing-identity.json"), "signing identity")
    require_exact_keys!(identity, %w[certificate_identity certificate_oidc_issuer schema_version], "signing identity")
    fail!("signing identity schema mismatch") unless identity["schema_version"] == SIGNING_IDENTITY_SCHEMA
    fail!("signing identity issuer mismatch") unless identity["certificate_oidc_issuer"] == OIDC_ISSUER
    pattern = %r{\Ahttps://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/\.github/workflows/release-artifact\.yml@refs/(?:heads|tags)/[A-Za-z0-9._/-]+\z}
    fail!("signing identity is not a pinned GitHub workflow identity") unless identity["certificate_identity"].match?(pattern)
    identity
  end

  def validate_sbom_value!(value)
    case value
    when Hash
      value.each do |key, nested|
        fail!("SBOM contains forbidden key #{key}") if FORBIDDEN_SBOM_KEYS.include?(key.downcase)
        validate_sbom_value!(nested)
      end
    when Array
      value.each { |nested| validate_sbom_value!(nested) }
    when String
      fail!("SBOM contains a local path or URL") if value.match?(LOCAL_REFERENCE)
    end
  end

  def validate_sbom!(directory, values)
    sbom = read_json(File.join(directory, "gateway-sbom.cdx.json"), "SBOM")
    fail!("SBOM format mismatch") unless sbom["bomFormat"] == "CycloneDX" && sbom["specVersion"] == "1.5"
    components = sbom["components"]
    fail!("SBOM has no components") unless components.is_a?(Array) && !components.empty?
    target_values = Array(sbom.dig("metadata", "properties")).select do |property|
      property["name"] == "cdx:rustc:sbom:target:triple"
    end.map { |property| property["value"] }
    fail!("SBOM target mismatch") unless target_values.include?(values[:target])
    validate_sbom_value!(sbom)
  end

  def tar_entries(path)
    entries = {}
    File.open(path, "rb") do |file|
      Gem::Package::TarReader.new(file) do |reader|
        reader.each do |entry|
          name = entry.full_name
          safe_name = name.match?(%r{\A(?:[A-Za-z0-9._-]+/)*[A-Za-z0-9._-]+\z}) && !name.split("/").include?("..")
          fail!("OCI archive contains an unsafe entry #{name.inspect}") unless safe_name
          fail!("OCI archive contains duplicate entry #{name}") if entries.key?(name)
          entries[name] = entry.read if entry.file?
        end
      end
    end
    entries
  rescue Gem::Package::TarInvalidError => error
    fail!("OCI archive is invalid: #{error.message}")
  end

  def blob(entries, descriptor, label)
    fail!("#{label} descriptor must be an object") unless descriptor.is_a?(Hash)
    %w[digest mediaType size].each do |key|
      fail!("#{label} descriptor is missing #{key}") unless descriptor.key?(key)
    end
    digest = descriptor["digest"]
    fail!("#{label} digest is not SHA-256") unless digest.is_a?(String) && digest.match?(/\Asha256:[0-9a-f]{64}\z/)
    path = "blobs/sha256/#{digest.delete_prefix("sha256:")}"
    bytes = entries.fetch(path) { fail!("#{label} blob is missing") }
    fail!("#{label} digest mismatch") unless sha256_text(bytes) == digest.delete_prefix("sha256:")
    fail!("#{label} size mismatch") unless bytes.bytesize == descriptor["size"]
    bytes
  end

  def validate_oci!(directory, values)
    archive = File.join(directory, "gateway-image.oci.tar")
    entries = tar_entries(archive)
    layout = JSON.parse(entries.fetch("oci-layout") { fail!("OCI archive has no oci-layout") })
    fail!("OCI layout version mismatch") unless layout["imageLayoutVersion"] == "1.0.0"
    index = JSON.parse(entries.fetch("index.json") { fail!("OCI archive has no index.json") })
    descriptors = index["manifests"]
    fail!("OCI archive must contain one image manifest") unless descriptors.is_a?(Array) && descriptors.length == 1
    manifest = JSON.parse(blob(entries, descriptors.first, "OCI manifest"))
    layers = manifest["layers"]
    fail!("OCI image has no filesystem layer") unless layers.is_a?(Array) && !layers.empty?
    layers.each_with_index { |layer, index_value| blob(entries, layer, "OCI layer #{index_value}") }
    config = JSON.parse(blob(entries, manifest.fetch("config") { fail!("OCI manifest has no config") }, "OCI config"))
    fail!("OCI architecture mismatch") unless config["architecture"] == "amd64" && config["os"] == "linux"
    image_config = config.fetch("config") { fail!("OCI config has no runtime configuration") }
    fail!("OCI image must run as non-root") unless image_config["User"] == "65532:65532"
    fail!("OCI entrypoint mismatch") unless image_config["Entrypoint"] == ["/usr/local/bin/gateway"]
    exposed_ports = image_config["ExposedPorts"]
    fail!("OCI image must not expose a listener") unless exposed_ports.nil? || exposed_ports.empty?
    labels = image_config.fetch("Labels") { fail!("OCI image has no labels") }
    expected_labels = {
      "org.opencontainers.image.revision" => values[:revision],
      "org.opencontainers.image.version" => values[:version],
      "org.opencontainers.image.rust.toolchain" => values[:rust_toolchain],
      "org.opencontainers.image.target" => values[:target]
    }
    expected_labels.each do |key, value|
      fail!("OCI label #{key} mismatch") unless labels[key] == value
    end
  rescue JSON::ParserError => error
    fail!("OCI archive has invalid JSON: #{error.message}")
  end

  def validate_payloads!(directory, values)
    entries = directory_entries(directory)
    permitted_entries = [PAYLOAD_NAMES.sort, (PAYLOAD_NAMES + [MANIFEST_NAME]).sort]
    fail!("artifact payload set mismatch") unless permitted_entries.include?(entries)
    validate_binary_metadata!(directory, values)
    validate_build_metadata!(directory, values)
    validate_signing_identity!(directory)
    validate_sbom!(directory, values)
    validate_oci!(directory, values)
  end

  def manifest_records(directory)
    PAYLOAD_NAMES.sort.map do |name|
      path = File.join(directory, name)
      { "name" => name, "bytes" => File.size(path), "sha256" => sha256(path) }
    end
  end

  def generate_manifest
    values = release_options("manifest", require_output: true)
    directory = File.expand_path(values[:artifact_dir])
    output = File.expand_path(values[:output])
    fail!("manifest: output must be inside the artifact directory") unless File.dirname(output) == directory
    fail!("manifest: output filename must be #{MANIFEST_NAME}") unless File.basename(output) == MANIFEST_NAME
    validate_payloads!(directory, values)
    manifest = {
      "schema_version" => MANIFEST_SCHEMA,
      "revision" => values[:revision],
      "rust_toolchain" => values[:rust_toolchain],
      "target" => values[:target],
      "files" => manifest_records(directory)
    }
    write_json(output, manifest)
    puts "p12-artifact: wrote #{output}"
  end

  def validate_manifest!(directory, values)
    manifest = read_json(File.join(directory, MANIFEST_NAME), "artifact manifest")
    require_exact_keys!(manifest, %w[files revision rust_toolchain schema_version target], "artifact manifest")
    fail!("artifact manifest schema mismatch") unless manifest["schema_version"] == MANIFEST_SCHEMA
    %i[revision rust_toolchain target].each do |key|
      fail!("artifact manifest #{key} mismatch") unless manifest[key.to_s] == values[key]
    end
    records = manifest["files"]
    fail!("artifact manifest files must be an array") unless records.is_a?(Array) && records.length == PAYLOAD_NAMES.length
    names = []
    records.each do |record|
      require_exact_keys!(record, %w[bytes name sha256], "artifact manifest file")
      name = record["name"]
      fail!("artifact manifest filename is invalid") unless name.is_a?(String) && name.match?(/\A[A-Za-z0-9][A-Za-z0-9._-]*\z/)
      fail!("artifact manifest size is invalid") unless record["bytes"].is_a?(Integer) && record["bytes"].positive?
      fail!("artifact manifest digest is invalid") unless record["sha256"].is_a?(String) && record["sha256"].match?(/\A[0-9a-f]{64}\z/)
      names << name
      path = File.join(directory, name)
      fail!("manifested artifact #{name} is missing") unless File.file?(path)
      fail!("manifested artifact #{name} size mismatch") unless File.size(path) == record["bytes"]
      fail!("manifested artifact #{name} digest mismatch") unless sha256(path) == record["sha256"]
    end
    fail!("artifact manifest names are not deterministic") unless names == PAYLOAD_NAMES.sort
  end

  def expected_entries(require_signature, require_receipt)
    entries = PAYLOAD_NAMES + [MANIFEST_NAME]
    entries += [SIGNATURE_NAME, BUNDLE_NAME] if require_signature
    entries << RECEIPT_NAME if require_receipt
    entries.sort
  end

  def validate_receipt!(directory, values, identity)
    receipt = read_json(File.join(directory, RECEIPT_NAME), "artifact receipt")
    require_exact_keys!(receipt, %w[files manifest_sha256 revision sbom_sha256 schema_version signing workflow_run], "artifact receipt")
    fail!("artifact receipt schema mismatch") unless receipt["schema_version"] == RECEIPT_SCHEMA
    fail!("artifact receipt revision mismatch") unless receipt["revision"] == values[:revision]
    fail!("artifact receipt workflow run is invalid") unless receipt["workflow_run"].match?(%r{\Ahttps://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/actions/runs/[0-9]+\z})
    signing = receipt["signing"]
    require_exact_keys!(signing, %w[certificate_identity certificate_oidc_issuer type verification], "artifact receipt signing")
    fail!("artifact receipt signing type mismatch") unless signing["type"] == "sigstore-keyless" && signing["verification"] == "verified"
    fail!("artifact receipt signing identity mismatch") unless signing["certificate_identity"] == identity["certificate_identity"]
    fail!("artifact receipt signing issuer mismatch") unless signing["certificate_oidc_issuer"] == OIDC_ISSUER
    expected = (PAYLOAD_NAMES + [MANIFEST_NAME, SIGNATURE_NAME, BUNDLE_NAME]).sort
    fail!("artifact receipt file set mismatch") unless receipt["files"].is_a?(Hash) && receipt["files"].keys.sort == expected
    receipt["files"].each do |name, digest|
      fail!("artifact receipt digest mismatch for #{name}") unless digest == sha256(File.join(directory, name))
    end
    fail!("artifact receipt manifest digest mismatch") unless receipt["manifest_sha256"] == sha256(File.join(directory, MANIFEST_NAME))
    fail!("artifact receipt SBOM digest mismatch") unless receipt["sbom_sha256"] == sha256(File.join(directory, "gateway-sbom.cdx.json"))
  end

  def verify_artifact
    values = release_options("verify") do |parser, result|
      parser.on("--require-signature") { result[:require_signature] = true }
      parser.on("--require-receipt") { result[:require_receipt] = true }
    end
    fail!("verify: --require-receipt requires --require-signature") if values[:require_receipt] && !values[:require_signature]
    directory = File.expand_path(values[:artifact_dir])
    expected = expected_entries(values[:require_signature], values[:require_receipt])
    fail!("artifact directory has missing or unexpected files") unless directory_entries(directory) == expected
    validate_binary_metadata!(directory, values)
    validate_build_metadata!(directory, values)
    identity = validate_signing_identity!(directory)
    validate_sbom!(directory, values)
    validate_oci!(directory, values)
    validate_manifest!(directory, values)
    validate_receipt!(directory, values, identity) if values[:require_receipt]
    puts "p12-artifact: verified #{directory}"
  end

  def generate_identity
    values = parse_options("identity") do |parser, result|
      parser.on("--repository OWNER/REPOSITORY", String) { |value| result[:repository] = value }
      parser.on("--workflow-path PATH", String) { |value| result[:workflow_path] = value }
      parser.on("--ref REF", String) { |value| result[:ref] = value }
      parser.on("--output PATH", String) { |value| result[:output] = value }
    end
    %i[repository workflow_path ref output].each { |key| required!(values, key, "identity") }
    fail!("identity: invalid repository") unless values[:repository].match?(%r{\A[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\z})
    fail!("identity: invalid workflow path") unless values[:workflow_path] == ".github/workflows/release-artifact.yml"
    fail!("identity: invalid Git ref") unless values[:ref].match?(%r{\Arefs/(?:heads|tags)/[A-Za-z0-9._/-]+\z})
    write_json(values[:output], {
      "schema_version" => SIGNING_IDENTITY_SCHEMA,
      "certificate_identity" => "https://github.com/#{values[:repository]}/#{values[:workflow_path]}@#{values[:ref]}",
      "certificate_oidc_issuer" => OIDC_ISSUER
    })
    puts "p12-artifact: wrote #{values[:output]}"
  end

  def generate_build_metadata
    values = parse_options("build-metadata") do |parser, result|
      parser.on("--revision SHA", String) { |value| result[:revision] = value }
      parser.on("--rust-toolchain VERSION", String) { |value| result[:rust_toolchain] = value }
      parser.on("--target TARGET", String) { |value| result[:target] = value }
      parser.on("--version VERSION", String) { |value| result[:version] = value }
      parser.on("--output PATH", String) { |value| result[:output] = value }
    end
    %i[revision rust_toolchain target version output].each { |key| required!(values, key, "build-metadata") }
    fail!("build-metadata: invalid revision") unless valid_revision?(values[:revision])
    fail!("build-metadata: invalid Rust toolchain") unless valid_toolchain?(values[:rust_toolchain])
    fail!("build-metadata: target must be #{TARGET}") unless values[:target] == TARGET
    fail!("build-metadata: invalid version") unless valid_version?(values[:version])
    write_json(values[:output], {
      "schema_version" => BUILD_METADATA_SCHEMA,
      "binary" => "gateway-x86_64-unknown-linux-gnu",
      "revision" => values[:revision],
      "rust_toolchain" => values[:rust_toolchain],
      "target" => values[:target],
      "version" => values[:version]
    })
    puts "p12-artifact: wrote #{values[:output]}"
  end

  def generate_receipt
    values = release_options("receipt", require_output: true) do |parser, result|
      parser.on("--workflow-run URL", String) { |value| result[:workflow_run] = value }
    end
    workflow_run = required!(values, :workflow_run, "receipt")
    directory = File.expand_path(values[:artifact_dir])
    output = File.expand_path(values[:output])
    fail!("receipt: output must be inside the artifact directory") unless File.dirname(output) == directory
    fail!("receipt: output filename must be #{RECEIPT_NAME}") unless File.basename(output) == RECEIPT_NAME
    fail!("receipt: workflow run is invalid") unless workflow_run.match?(%r{\Ahttps://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/actions/runs/[0-9]+\z})
    fail!("receipt: artifact directory has missing or unexpected files") unless directory_entries(directory) == expected_entries(true, false)
    validate_binary_metadata!(directory, values)
    validate_build_metadata!(directory, values)
    identity = validate_signing_identity!(directory)
    validate_sbom!(directory, values)
    validate_oci!(directory, values)
    validate_manifest!(directory, values)
    files = (PAYLOAD_NAMES + [MANIFEST_NAME, SIGNATURE_NAME, BUNDLE_NAME]).sort.to_h do |name|
      [name, sha256(File.join(directory, name))]
    end
    write_json(output, {
      "schema_version" => RECEIPT_SCHEMA,
      "revision" => values[:revision],
      "workflow_run" => workflow_run,
      "manifest_sha256" => files.fetch(MANIFEST_NAME),
      "sbom_sha256" => files.fetch("gateway-sbom.cdx.json"),
      "signing" => {
        "type" => "sigstore-keyless",
        "verification" => "verified",
        "certificate_identity" => identity.fetch("certificate_identity"),
        "certificate_oidc_issuer" => identity.fetch("certificate_oidc_issuer")
      },
      "files" => files
    })
    puts "p12-artifact: wrote #{output}"
  end

  def inspect_oci
    values = release_options("inspect-oci")
    directory = File.expand_path(values[:artifact_dir])
    validate_oci!(directory, values)
    puts "p12-artifact: OCI metadata verified #{File.join(directory, "gateway-image.oci.tar")}"
  end

  def run
    command = ARGV.shift
    case command
    when "manifest" then generate_manifest
    when "verify" then verify_artifact
    when "identity" then generate_identity
    when "build-metadata" then generate_build_metadata
    when "receipt" then generate_receipt
    when "inspect-oci" then inspect_oci
    else
      fail!("usage: #{$PROGRAM_NAME} [manifest|verify|identity|build-metadata|receipt|inspect-oci] [options]")
    end
  end
end

begin
  P12ReleaseArtifact.run
rescue P12ReleaseArtifact::Failure => error
  warn "p12-artifact: #{error.message}"
  exit 1
end
