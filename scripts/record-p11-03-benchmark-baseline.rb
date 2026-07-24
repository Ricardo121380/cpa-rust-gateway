#!/usr/bin/env ruby

# Converts Criterion raw samples plus externally captured peak RSS into P11-03's reviewed,
# portable candidate format. This script never changes benchmarks/baseline.json itself.

require "json"
require "optparse"
require "time"

ROOT = File.expand_path("..", __dir__)
BENCHMARKS = {
  "mock_provider_canonical_drain" => "p11_03_mock_provider_canonical_drain/zero_delay_text_lifecycle",
  "http_responses_warm_path" => "p11_03_http_responses_warm_path/non_streaming_text_response"
}.freeze

options = {
  criterion_root: nil,
  mock_rss_bytes: nil,
  http_rss_bytes: nil,
  output: nil
}

parser = OptionParser.new do |arguments|
  arguments.banner = "usage: #{$PROGRAM_NAME} --criterion-root PATH --mock-rss-bytes BYTES --http-rss-bytes BYTES --output PATH"
  arguments.on("--criterion-root PATH", "Criterion result directory") { |value| options[:criterion_root] = value }
  arguments.on("--mock-rss-bytes BYTES", Integer, "peak RSS for the Mock Provider command") { |value| options[:mock_rss_bytes] = value }
  arguments.on("--http-rss-bytes BYTES", Integer, "peak RSS for the HTTP command") { |value| options[:http_rss_bytes] = value }
  arguments.on("--output PATH", "candidate JSON output path") { |value| options[:output] = value }
end
parser.parse!
abort parser.to_s unless ARGV.empty?
abort parser.to_s if options.values.any?(&:nil?)

def positive_integer(value, label)
  abort "p11-03 baseline: #{label} must be a positive integer" unless value.is_a?(Integer) && value.positive?

  value
end

def percentile(samples, fraction)
  index = ((samples.length - 1) * fraction).ceil
  samples.fetch(index)
end

def read_benchmark(criterion_root, criterion_path, rss_bytes)
  path = File.join(criterion_root, criterion_path, "new", "sample.json")
  abort "p11-03 baseline: missing Criterion sample #{criterion_path}" unless File.file?(path)

  sample = JSON.parse(File.read(path, encoding: "UTF-8"))
  iterations = sample.fetch("iters")
  elapsed = sample.fetch("times")
  valid_shape = iterations.is_a?(Array) && elapsed.is_a?(Array) &&
                iterations.length == elapsed.length && !iterations.empty?
  abort "p11-03 baseline: malformed Criterion sample #{criterion_path}" unless valid_shape

  per_operation = iterations.zip(elapsed).map do |iteration_count, total_nanoseconds|
    valid_value = iteration_count.is_a?(Numeric) && total_nanoseconds.is_a?(Numeric) &&
                  iteration_count.positive? && total_nanoseconds.positive?
    abort "p11-03 baseline: invalid Criterion sample values #{criterion_path}" unless valid_value

    total_nanoseconds.to_f / iteration_count.to_f
  end.sort
  p50 = percentile(per_operation, 0.50)
  p99 = percentile(per_operation, 0.99)
  abort "p11-03 baseline: invalid measured latency #{criterion_path}" unless p50.positive? && p99.positive?

  {
    "p50_ns" => p50.round,
    "p99_ns" => p99.round,
    "throughput_ops_per_sec" => (1_000_000_000.0 / p50).round,
    "max_rss_bytes" => positive_integer(rss_bytes, "RSS")
  }
end

def command_output(command)
  output = IO.popen(command, chdir: ROOT, &:read).strip
  abort "p11-03 baseline: command failed: #{command.join(" ")}" unless $?.success?

  output
end

criterion_root = File.expand_path(options.fetch(:criterion_root))
revision = command_output(["git", "rev-parse", "HEAD"])
rustc = command_output(["rustc", "-V"])
machine = command_output(["uname", "-m"])
operating_system = command_output(["uname", "-s"])

measurements = BENCHMARKS.map do |identifier, criterion_path|
  rss_bytes = identifier == "mock_provider_canonical_drain" ? options.fetch(:mock_rss_bytes) : options.fetch(:http_rss_bytes)
  read_benchmark(criterion_root, criterion_path, rss_bytes).merge("id" => identifier)
end

payload = {
  "schema_version" => 1,
  "recorded_at" => Time.now.utc.iso8601,
  "git_revision" => revision,
  "environment" => {
    "os" => operating_system,
    "arch" => machine,
    "rustc" => rustc
  },
  "method" => {
    "tool" => "criterion",
    "sample_size" => 30,
    "warmup_seconds" => 1,
    "measurement_seconds" => 3,
    "latency_source" => "Criterion raw per-operation samples",
    "rss_source" => "per-command operating-system peak RSS"
  },
  "thresholds" => {
    "max_p99_growth_ratio" => 1.15,
    "max_rss_growth_ratio" => 1.15,
    "min_throughput_ratio" => 0.90,
    "local_http_p99_ns" => 5_000_000
  },
  "benchmarks" => measurements
}

output = File.expand_path(options.fetch(:output))
File.write(output, "#{JSON.pretty_generate(payload)}\n", encoding: "UTF-8")
puts "p11-03 baseline candidate: #{output}"
