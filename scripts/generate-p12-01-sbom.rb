#!/usr/bin/env ruby
# frozen_string_literal: true

# Generates the P12 artifact SBOM outside the checkout and removes every local path, URI and
# external URL. The release artifact needs dependency identity, not a workstation location or an
# upstream/package endpoint inventory.

require "fileutils"
require "json"
require "open3"
require "optparse"

REPOSITORY_ROOT = File.expand_path("..", __dir__)
RAW_OVERRIDE_FILENAME = ".p12-01-cyclonedx-raw"
RAW_FILENAME = "#{RAW_OVERRIDE_FILENAME}.json"
RAW_GLOBS = ["{apps,crates}/**/#{RAW_FILENAME}", "{apps,crates}/**/#{RAW_FILENAME}.json"].freeze
LOCAL_REFERENCE = %r{(?:path\+file:|file://|https?://|/Users/|/home/|[A-Za-z]:\\\\Users\\)}i
FORBIDDEN_KEYS = %w[
  authorization cookie credential endpoint password secret token url
].freeze

def fail_with(message)
  warn "p12-01-sbom: #{message}"
  exit 1
end

def command_output(*command)
  output, status = Open3.capture2e(*command)
  fail_with("command failed: #{command.join(" ")}\n#{output}") unless status.success?

  output
end

def source_date_epoch
  supplied = ENV.fetch("SOURCE_DATE_EPOCH", "").strip
  return supplied unless supplied.empty?

  epoch = command_output(
    "git", "-C", REPOSITORY_ROOT, "log", "-1", "--format=%ct", "--", "Cargo.lock", "apps/gateway/Cargo.toml"
  ).strip
  fail_with("no dependency-source commit timestamp") if epoch.empty?

  epoch
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

def redact_metadata!(value)
  case value
  when Hash
    value.delete("externalReferences")
    value.delete("url")
    value.each_value { |nested| redact_metadata!(nested) }
  when Array
    value.each { |nested| redact_metadata!(nested) }
  end
end

def assert_redacted!(value)
  case value
  when Hash
    value.each do |key, nested|
      fail_with("forbidden SBOM key remains: #{key}") if FORBIDDEN_KEYS.include?(key.downcase)
      assert_redacted!(nested)
    end
  when Array
    value.each { |nested| assert_redacted!(nested) }
  when String
    fail_with("local path or endpoint remains after sanitization") if value.match?(LOCAL_REFERENCE)
  end
end

options = { target: "x86_64-unknown-linux-gnu" }
OptionParser.new do |parser|
  parser.banner = "usage: #{$PROGRAM_NAME} --output PATH [--target TARGET]"
  parser.on("--output PATH", String) { |value| options[:output] = value }
  parser.on("--target TARGET", String) { |value| options[:target] = value }
end.parse!
fail_with("unexpected arguments") unless ARGV.empty?
fail_with("--output is required") if options[:output].to_s.empty?
fail_with("target must be x86_64-unknown-linux-gnu") unless options[:target] == "x86_64-unknown-linux-gnu"

output_path = File.expand_path(options[:output], Dir.pwd)

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
    "--target", options[:target],
    "--override-filename", RAW_OVERRIDE_FILENAME,
    chdir: REPOSITORY_ROOT
  )
  fail_with("cargo-cyclonedx failed") unless generated

  raw_path = File.join(REPOSITORY_ROOT, "apps/gateway", RAW_FILENAME)
  fail_with("gateway SBOM was not generated") unless File.file?(raw_path)

  bom = JSON.parse(File.read(raw_path))
  components = []
  walk_component(bom.dig("metadata", "component"), components) if bom.dig("metadata", "component")
  bom.fetch("components").each { |component| walk_component(component, components) }

  replacements = {}
  used_references = {}
  components.each_with_index do |component, index|
    purl = stable_purl(component)
    component["purl"] = purl
    reference = used_references.key?(purl) ? "#{purl}#target-#{index}" : purl
    used_references[reference] = true
    replacements[component.fetch("bom-ref")] = reference
    component["bom-ref"] = reference
  end

  rewrite_strings!(bom, replacements)
  redact_metadata!(bom)
  fail_with("components are empty") if bom.fetch("components").empty?
  fail_with("wrong CycloneDX format") unless bom.fetch("bomFormat") == "CycloneDX" && bom.fetch("specVersion") == "1.5"
  assert_redacted!(bom)

  FileUtils.mkdir_p(File.dirname(output_path))
  File.write(output_path, "#{JSON.pretty_generate(bom)}\n")
  puts "p12-01-sbom: wrote #{output_path}"
ensure
  FileUtils.rm_f(raw_paths)
end
