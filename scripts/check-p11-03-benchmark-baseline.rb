#!/usr/bin/env ruby

# Compares one newly measured P11-03 candidate with the approved baseline. The baseline's
# threshold and benchmark identity are authoritative; malformed input and missing values fail
# closed rather than being ignored.

require "json"
require "optparse"
require "time"

ROOT = File.expand_path("..", __dir__)
BENCHMARK_IDS = %w[mock_provider_canonical_drain http_responses_warm_path].freeze
DOCUMENT_KEYS = %w[benchmarks environment git_revision method recorded_at schema_version thresholds].freeze
BENCHMARK_KEYS = %w[id max_rss_bytes p50_ns p99_ns throughput_ops_per_sec].freeze
THRESHOLD_KEYS = %w[local_http_p99_ns max_p99_growth_ratio max_rss_growth_ratio min_throughput_ratio].freeze

options = {
  baseline: File.join(ROOT, "benchmarks", "baseline.json"),
  candidate: nil
}

parser = OptionParser.new do |arguments|
  arguments.banner = "usage: #{$PROGRAM_NAME} [--baseline PATH] --candidate PATH"
  arguments.on("--baseline PATH", "approved baseline JSON") { |value| options[:baseline] = value }
  arguments.on("--candidate PATH", "new benchmark candidate JSON") { |value| options[:candidate] = value }
end
parser.parse!
abort parser.to_s unless ARGV.empty?
abort parser.to_s if options[:candidate].nil?

def parse_document(path, label)
  abort "p11-03 comparator: missing #{label}: #{path}" unless File.file?(path)

  JSON.parse(File.read(path, encoding: "UTF-8"))
rescue JSON::ParserError
  abort "p11-03 comparator: invalid JSON for #{label}"
end

def positive_integer?(value)
  value.is_a?(Integer) && value.positive?
end

def positive_number?(value)
  value.is_a?(Numeric) && value.positive?
end

def validate_document(document, label)
  errors = []
  errors << "#{label}: document must be an object" unless document.is_a?(Hash)
  return errors unless document.is_a?(Hash)

  errors << "#{label}: document keys differ" unless document.keys.sort == DOCUMENT_KEYS
  errors << "#{label}: unsupported schema" unless document["schema_version"] == 1
  begin
    Time.iso8601(document.fetch("recorded_at"))
  rescue ArgumentError, KeyError, TypeError
    errors << "#{label}: invalid recorded_at"
  end
  valid_revision = document["git_revision"].is_a?(String) &&
                   document["git_revision"].match?(/\A[0-9a-f]{40}\z/)
  errors << "#{label}: invalid git revision" unless valid_revision

  environment = document["environment"]
  valid_environment = environment.is_a?(Hash) &&
                      %w[arch os rustc].all? { |key| environment[key].is_a?(String) && !environment[key].empty? }
  errors << "#{label}: invalid environment" unless valid_environment

  method = document["method"]
  valid_method = method.is_a?(Hash) && method["tool"] == "criterion" &&
                 method["sample_size"] == 30 && method["warmup_seconds"] == 1 &&
                 method["measurement_seconds"] == 3 && method["latency_source"].is_a?(String) &&
                 method["rss_source"].is_a?(String)
  errors << "#{label}: invalid method" unless valid_method

  thresholds = document["thresholds"]
  valid_thresholds = thresholds.is_a?(Hash) && thresholds.keys.sort == THRESHOLD_KEYS &&
                     positive_number?(thresholds["max_p99_growth_ratio"]) &&
                     positive_number?(thresholds["max_rss_growth_ratio"]) &&
                     positive_number?(thresholds["min_throughput_ratio"]) &&
                     positive_integer?(thresholds["local_http_p99_ns"])
  errors << "#{label}: invalid thresholds" unless valid_thresholds

  benchmarks = document["benchmarks"]
  unless benchmarks.is_a?(Array) && benchmarks.length == BENCHMARK_IDS.length
    errors << "#{label}: invalid benchmark set"
    return errors
  end

  identifiers = benchmarks.map { |benchmark| benchmark.is_a?(Hash) ? benchmark["id"] : nil }
  errors << "#{label}: benchmark identity differs" unless identifiers.sort == BENCHMARK_IDS.sort
  benchmarks.each do |benchmark|
    valid_measurement = benchmark.is_a?(Hash) && benchmark.keys.sort == BENCHMARK_KEYS &&
                        benchmark["id"].is_a?(String) && positive_integer?(benchmark["p50_ns"]) &&
                        positive_integer?(benchmark["p99_ns"]) &&
                        positive_integer?(benchmark["throughput_ops_per_sec"]) &&
                        positive_integer?(benchmark["max_rss_bytes"])
    errors << "#{label}: invalid benchmark measurement" unless valid_measurement
  end
  errors
end

baseline = parse_document(File.expand_path(options.fetch(:baseline)), "baseline")
candidate = parse_document(File.expand_path(options.fetch(:candidate)), "candidate")
errors = validate_document(baseline, "baseline") + validate_document(candidate, "candidate")

if errors.empty?
  errors << "candidate: environment differs from approved local baseline" unless candidate.fetch("environment") == baseline.fetch("environment")
  errors << "candidate: method differs from approved baseline" unless candidate.fetch("method") == baseline.fetch("method")
  errors << "candidate: thresholds differ from approved baseline" unless candidate.fetch("thresholds") == baseline.fetch("thresholds")
end

if errors.empty?
  threshold = baseline.fetch("thresholds")
  baseline_by_id = baseline.fetch("benchmarks").to_h { |benchmark| [benchmark.fetch("id"), benchmark] }
  candidate.fetch("benchmarks").each do |measurement|
    identifier = measurement.fetch("id")
    approved = baseline_by_id.fetch(identifier)
    p99_limit = (approved.fetch("p99_ns") * threshold.fetch("max_p99_growth_ratio")).floor
    rss_limit = (approved.fetch("max_rss_bytes") * threshold.fetch("max_rss_growth_ratio")).floor
    throughput_floor = (approved.fetch("throughput_ops_per_sec") * threshold.fetch("min_throughput_ratio")).ceil
    errors << "#{identifier}: P99 regression exceeds approved limit" if measurement.fetch("p99_ns") > p99_limit
    errors << "#{identifier}: RSS regression exceeds approved limit" if measurement.fetch("max_rss_bytes") > rss_limit
    errors << "#{identifier}: throughput regression exceeds approved limit" if measurement.fetch("throughput_ops_per_sec") < throughput_floor
  end

  local_http = candidate.fetch("benchmarks").find { |measurement| measurement.fetch("id") == "http_responses_warm_path" }
  errors << "http_responses_warm_path: local absolute P99 exceeds 5 ms" if local_http.fetch("p99_ns") > threshold.fetch("local_http_p99_ns")
end

if errors.empty?
  puts "p11-03 comparator: pass (2 benchmarks, P99/RSS/throughput thresholds)"
else
  warn errors.join("\n")
  exit 1
end
