#!/usr/bin/env ruby
# frozen_string_literal: true

# The Canary split lives in front of the data plane, so its timeouts are only correct relative to
# the gateway's own transport ceilings. Those live in Rust constants, which drift independently of a
# Caddyfile. This reads both sides and fails closed when they stop agreeing, and it re-asserts the
# exposure invariants `CR-P12-ROLLOUT-001` made review items.

require "optparse"

ROOT = File.expand_path("..", __dir__)
DEFAULT_SPLIT = File.join(ROOT, "deploy", "caddy", "canary.Caddyfile")
DEFAULT_ROLLBACK = File.join(ROOT, "deploy", "caddy", "rollback.Caddyfile")
DEFAULT_STAGING = File.join(ROOT, "deploy", "caddy", "staging-domain.Caddyfile")
HTTP_SOURCE = File.join(ROOT, "crates", "gateway-http-actix", "src", "lib.rs")
RUNTIME_SOURCE = File.join(ROOT, "apps", "gateway", "src", "runtime.rs")

DATA_PLANE = "127.0.0.1:18180"
MANAGEMENT_PLANE = "127.0.0.1:18181"
INCUMBENT = "127.0.0.1:8317"

options = { split: DEFAULT_SPLIT, rollback: DEFAULT_ROLLBACK, staging: DEFAULT_STAGING }
OptionParser.new do |parser|
  parser.on("--split PATH", "Validate an alternate split fragment") { |path| options[:split] = path }
  parser.on("--rollback PATH", "Validate an alternate rollback fragment") { |path| options[:rollback] = path }
  parser.on("--staging PATH", "Validate an alternate staging-domain fragment") { |path| options[:staging] = path }
end.parse!

errors = []

def read_source(path, label, errors)
  File.read(path, encoding: "UTF-8")
rescue Errno::ENOENT
  errors << "#{label} is unavailable: #{path}"
  nil
end

split = read_source(options[:split], "Canary split fragment", errors)
rollback = read_source(options[:rollback], "rollback fragment", errors)
staging = read_source(options[:staging], "staging-domain fragment", errors)
http_source = read_source(HTTP_SOURCE, "HTTP crate", errors)
runtime_source = read_source(RUNTIME_SOURCE, "P12 runtime", errors)

if [split, rollback, staging, http_source, runtime_source].any?(&:nil?)
  errors.each { |error| warn "p12-caddy-split: #{error}" }
  exit 1
end

DURATION_UNITS = { "s" => 1, "m" => 60, "h" => 3600 }.freeze

def parse_duration(value)
  match = value.to_s.match(/\A(\d+)(s|m|h)\z/)
  return nil unless match

  match[1].to_i * DURATION_UNITS.fetch(match[2])
end

def caddy_timeout(text, name)
  match = text.match(/^\s*#{name}\s+(\S+)\s*$/)
  match && match[1]
end

# Gateway-side ceilings, read from the code rather than restated here.
keepalive = http_source.match(/SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs\((\d+)\)/)
body_timeout = http_source.match(/INFERENCE_REQUEST_BODY_TIMEOUT: Duration = Duration::from_secs\((\d+)\)/)
streaming_total = runtime_source.match(/P12_STREAMING_TOTAL_TIMEOUT: Duration = Duration::from_hours\((\d+)\)/)
progress = runtime_source.match(/P12_STREAMING_PROGRESS_TIMEOUT: Duration = Duration::from_mins\((\d+)\)/)

errors << "cannot read SSE_KEEPALIVE_INTERVAL from the HTTP crate" unless keepalive
errors << "cannot read INFERENCE_REQUEST_BODY_TIMEOUT from the HTTP crate" unless body_timeout
errors << "cannot read P12_STREAMING_TOTAL_TIMEOUT from the P12 runtime" unless streaming_total
errors << "cannot read P12_STREAMING_PROGRESS_TIMEOUT from the P12 runtime" unless progress

if keepalive && body_timeout && streaming_total && progress
  keepalive_seconds = keepalive[1].to_i
  body_seconds = body_timeout[1].to_i
  total_seconds = streaming_total[1].to_i * 3600
  progress_seconds = progress[1].to_i * 60

  # Caddy scopes a `servers` block by listener address, not by site. The server runs every site on
  # one :443 listener, so a global block here would silently retime unrelated sites. Omitting it is
  # safe: server-level timeouts default to none, which is what a long-lived event stream needs and
  # which satisfies CR-P12-ROLLOUT-001's ">15s" requirement by construction.
  directives = split.lines.reject { |line| line.strip.start_with?("#") }.join
  if directives.match?(/^\s*servers\b/)
    errors << "the split fragment must not declare a global servers block; it would apply to every " \
              "site sharing the :443 listener, not just this one"
  end

  read_header = caddy_timeout(split, "read_header")
  read_body = caddy_timeout(split, "read_body")
  write_timeout = caddy_timeout(split, "write")
  idle = caddy_timeout(split, "idle")

  # No timeout is the correct default. But if one is ever introduced, it must still clear the
  # gateway's own ceilings rather than cutting streams the gateway considers live.
  unless read_header.nil?
    header_seconds = parse_duration(read_header)
    if header_seconds.nil?
      errors << "read_header must be a plain <n>s/<n>m/<n>h duration"
    elsif header_seconds <= keepalive_seconds
      errors << "read_header #{read_header} is not above the #{keepalive_seconds}s SSE keepalive interval"
    end
  end
  unless read_body.nil?
    body_configured = parse_duration(read_body)
    if body_configured.nil?
      errors << "read_body must be a plain <n>s/<n>m/<n>h duration"
    elsif body_configured != body_seconds
      errors << "read_body #{read_body} does not match the gateway's #{body_seconds}s inbound body timeout"
    end
  end
  unless write_timeout.nil?
    write_seconds = parse_duration(write_timeout)
    if write_seconds.nil?
      errors << "write must be a plain <n>s/<n>m/<n>h duration"
    elsif !write_seconds.zero?
      errors << "write must stay 0s; a write deadline would truncate long-lived SSE responses"
    end
  end
  unless idle.nil?
    idle_seconds = parse_duration(idle)
    if idle_seconds.nil?
      errors << "idle must be a plain <n>s/<n>m/<n>h duration"
    else
      if idle_seconds <= keepalive_seconds
        errors << "idle #{idle} is not above the #{keepalive_seconds}s SSE keepalive interval"
      end
      if idle_seconds < progress_seconds
        errors << "idle #{idle} is below the gateway's #{progress_seconds}s streaming progress deadline"
      end
      if idle_seconds != total_seconds
        errors << "idle #{idle} does not match the gateway's #{total_seconds}s streaming total ceiling"
      end
    end
  end
end

# Exposure invariants from CR-P12-ROLLOUT-001. Comments are excluded: both fragments explain the
# management-plane rule in prose, and naming the port in a comment is not a route.
[options[:split], options[:rollback], options[:staging]].zip([split, rollback, staging]).each do |path, text|
  label = File.basename(path)
  directives = text.lines.reject { |line| line.strip.start_with?("#") }.join
  if directives.include?(MANAGEMENT_PLANE)
    errors << "#{label} must never route to the management listener #{MANAGEMENT_PLANE}"
  end
  # `encode` reintroduces buffering in front of an event stream.
  if directives.match?(/^\s*encode\s/)
    errors << "#{label} must not compress the data plane; encode reintroduces SSE buffering"
  end
  # Caddy scopes `servers` by listener address, so any of these fragments could retime every
  # co-located site.
  if directives.match?(/^\s*servers\b/)
    errors << "#{label} must not declare a global servers block; it would apply to every site " \
              "sharing the :443 listener"
  end
end

# The staging domain exists only to verify exposure before the split, so it must reach the data
# plane and nothing else.
staging_directives = staging.lines.reject { |line| line.strip.start_with?("#") }.join
unless staging_directives.include?(DATA_PLANE)
  errors << "the staging-domain fragment must reverse_proxy to #{DATA_PLANE}"
end
if staging_directives.include?(INCUMBENT)
  errors << "the staging-domain fragment must not route to the incumbent; it is not a split"
end

unless split.include?(DATA_PLANE)
  errors << "the split fragment must reverse_proxy the canary to #{DATA_PLANE}"
end
unless split.include?(INCUMBENT)
  errors << "the split fragment must keep unmatched traffic on the incumbent #{INCUMBENT}"
end

# The split must key on the non-secret literal prefix, on both accepted headers.
unless split.include?('header Authorization "Bearer rgw_*"')
  errors << "the split fragment must match the Authorization bearer prefix rgw_"
end
unless split.include?('header X-Api-Key "rgw_*"')
  errors << "the split fragment must match the x-api-key prefix rgw_"
end

# A key value in a reviewed config file would be a secret leak into version control.
if split.match?(/rgw_[A-Za-z0-9]/)
  errors << "the split fragment contains something longer than the bare rgw_ prefix"
end

# Rollback must remove the gateway from the path entirely, or it is not a rollback.
if rollback.include?(DATA_PLANE)
  errors << "the rollback fragment must not route any traffic to the new gateway"
end
unless rollback.include?(INCUMBENT)
  errors << "the rollback fragment must send the production hostname to #{INCUMBENT}"
end

# Both fragments must address the same hostname, or a reload would not actually swap the route.
split_hosts = split.scan(/^([a-z0-9.-]+\.[a-z]{2,})\s+\{/).flatten.uniq
rollback_hosts = rollback.scan(/^([a-z0-9.-]+\.[a-z]{2,})\s+\{/).flatten.uniq
if split_hosts != rollback_hosts
  errors << "split hosts #{split_hosts.inspect} and rollback hosts #{rollback_hosts.inspect} differ"
end

if errors.empty?
  puts "p12-caddy-split: ok (timeouts agree with the gateway; management plane unexposed; " \
       "rollback removes the gateway)"
else
  errors.each { |error| warn "p12-caddy-split: #{error}" }
  exit 1
end
