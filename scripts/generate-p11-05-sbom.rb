#!/usr/bin/env ruby
# frozen_string_literal: true

# Generates the release-candidate Rust CycloneDX SBOM without embedding the workstation path.
# cargo-cyclonedx emits `path+file:///...` bom-refs for workspace packages and a `file://`
# download_url qualifier for their PURLs. Those local implementation details are not part of a
# distributable SBOM, so this generator replaces every component ref with its stable Cargo PURL,
# rewrites dependency references, and rejects remaining file/path references before publishing.

require "fileutils"
require "json"
require "open3"

REPOSITORY_ROOT = File.expand_path("..", __dir__)
RAW_OVERRIDE_FILENAME = ".p11-05-cyclonedx-raw"
RAW_FILENAME = "#{RAW_OVERRIDE_FILENAME}.json"
RAW_GLOBS = [
  "{apps,crates}/**/#{RAW_FILENAME}",
  # Clean a failed first-run artifact made before the generator owned the filename.
  "{apps,crates}/**/p11-05-rust-sbom.json",
  "{apps,crates}/**/#{RAW_FILENAME}.json"
].freeze
OUTPUT_PATH = File.join(REPOSITORY_ROOT, "docs/reports/evidence/p11-05-rust-sbom.cdx.json")
LOCAL_REFERENCE = %r{(?:path\+file:|file://|/Users/|/home/)}i

def command_output(*command)
  output, status = Open3.capture2e(*command)
  abort("p11-05-sbom: command failed: #{command.join(" ")}\n#{output}") unless status.success?

  output
end

def source_date_epoch
  supplied = ENV.fetch("SOURCE_DATE_EPOCH", "").strip
  return supplied unless supplied.empty?

  command_output(
    "git", "-C", REPOSITORY_ROOT, "log", "-1", "--format=%ct", "--", "Cargo.lock", "apps/gateway/Cargo.toml"
  ).strip.then do |epoch|
    abort("p11-05-sbom: no dependency-source commit timestamp") if epoch.empty?

    epoch
  end
end

def walk_component(component, components)
  components << component
  component.fetch("components", []).each { |child| walk_component(child, components) }
end

def stable_purl(component)
  purl = component.fetch("purl", "").sub(/\?download_url=file:\/\/.*\z/, "")
  return purl unless purl.empty?

  name = component.fetch("name")
  version = component.fetch("version")
  "pkg:cargo/#{name}@#{version}"
end

def rewrite_strings!(value, replacements)
  case value
  when Hash
    value.each { |key, nested| value[key] = rewrite_strings!(nested, replacements) }
  when Array
    value.map! { |nested| rewrite_strings!(nested, replacements) }
  when String
    replacements.fetch(value, value)
  else
    value
  end
end

def assert_publishable!(bom)
  serialized = JSON.generate(bom)
  abort("p11-05-sbom: local path or file URI remains after sanitization") if serialized.match?(LOCAL_REFERENCE)
  abort("p11-05-sbom: components are empty") if bom.fetch("components").empty?
end

def raw_paths
  RAW_GLOBS.flat_map { |glob| Dir.glob(File.join(REPOSITORY_ROOT, glob)) }.uniq
end

FileUtils.rm_f(raw_paths)

begin
  generated = system(
    { "SOURCE_DATE_EPOCH" => source_date_epoch },
    "cargo", "cyclonedx",
    "--manifest-path", "apps/gateway/Cargo.toml",
    "--format", "json",
    "--spec-version", "1.5",
    "--all-features",
    "--target", "x86_64-unknown-linux-gnu",
    "--override-filename", RAW_OVERRIDE_FILENAME,
    chdir: REPOSITORY_ROOT
  )
  abort("p11-05-sbom: cargo-cyclonedx failed") unless generated

  raw_path = File.join(REPOSITORY_ROOT, "apps/gateway", RAW_FILENAME)
  abort("p11-05-sbom: gateway SBOM was not generated") unless File.file?(raw_path)

  bom = JSON.parse(File.read(raw_path))
  components = []
  walk_component(bom.dig("metadata", "component"), components) if bom.dig("metadata", "component")
  bom.fetch("components").each { |component| walk_component(component, components) }

  replacements = {}
  used_references = {}
  components.each_with_index do |component, index|
    purl = stable_purl(component)
    component["purl"] = purl
    stable_reference = purl
    stable_reference = "#{purl}#target-#{index}" if used_references.key?(stable_reference)
    used_references[stable_reference] = true
    replacements[component.fetch("bom-ref")] = stable_reference
    component["bom-ref"] = stable_reference
  end

  rewrite_strings!(bom, replacements)
  assert_publishable!(bom)
  File.write(OUTPUT_PATH, "#{JSON.pretty_generate(bom)}\n")
  puts "p11-05-sbom: wrote #{OUTPUT_PATH.delete_prefix("#{REPOSITORY_ROOT}/")}"
ensure
  FileUtils.rm_f(raw_paths)
end
