#!/usr/bin/env ruby

# Validates the value-free receipt produced by the P11-04 opt-in loopback soak.
# It intentionally accepts no request, response, endpoint, or credential fields.

abort("usage: #{$PROGRAM_NAME} ABSOLUTE_RECEIPT_PATH") unless ARGV.length == 1

path = ARGV.fetch(0)
abort("p11-04 receipt: path must be absolute") unless path.start_with?("/")
abort("p11-04 receipt: receipt does not exist") unless File.file?(path)

STATUS_FIELDS = %w[timestamp_unix state elapsed_seconds batches streams rss_bytes].freeze
RUNNER_FIELDS = %w[runner_state mode duration_seconds].freeze
RSS_WARM_UP_SAMPLES = 2
RSS_GROWTH_WINDOW_SAMPLES = 6
EXPECTED_DURATION_SECONDS = 24 * 60 * 60
EXPECTED_CONCURRENCY = 4

errors = []
events = []

File.foreach(path, encoding: "UTF-8").with_index(1) do |raw_line, line_number|
  line = raw_line.strip
  next if line.empty?

  fields = {}
  line.split.each do |token|
    key, value = token.split("=", 2)
    if key.nil? || value.nil? || key.empty? || value.empty? || fields.key?(key)
      errors << "line #{line_number}: malformed or duplicate field"
      next
    end
    fields[key] = value
  end
  next unless errors.empty? || errors.last !~ /line #{line_number}:/

  if fields.key?("runner_state")
    if fields.keys.sort != RUNNER_FIELDS.sort
      errors << "line #{line_number}: runner fields are not exact"
      next
    end
    events << { type: :runner, fields: fields, line_number: line_number }
  else
    if fields.keys.sort != STATUS_FIELDS.sort
      errors << "line #{line_number}: status fields are not exact"
      next
    end
    events << { type: :status, fields: fields, line_number: line_number }
  end
end

statuses = events.select { |event| event.fetch(:type) == :status }
runners = events.select { |event| event.fetch(:type) == :runner }
errors << "receipt has no status records" if statuses.empty?
errors << "receipt must contain exactly one runner record" unless runners.length == 1
errors << "runner record must be terminal" if !events.empty? && events.last.fetch(:type) != :runner

previous_timestamp = nil
previous_elapsed = nil
statuses.each_with_index do |event, index|
  fields = event.fetch(:fields)
  line_number = event.fetch(:line_number)
  numeric = {}
  %w[timestamp_unix elapsed_seconds batches streams rss_bytes].each do |key|
    value = fields.fetch(key)
    unless value.match?(/\A\d+\z/)
      errors << "line #{line_number}: #{key} must be an unsigned integer"
      next
    end
    numeric[key] = Integer(value, 10)
  end
  next unless numeric.length == 5

  if numeric.fetch("timestamp_unix").zero? || numeric.fetch("rss_bytes").zero?
    errors << "line #{line_number}: timestamp and RSS must be positive"
  end
  errors << "line #{line_number}: batches must be positive" if numeric.fetch("batches").zero?
  unless numeric.fetch("streams") == numeric.fetch("batches") * EXPECTED_CONCURRENCY
    errors << "line #{line_number}: streams must equal batches times #{EXPECTED_CONCURRENCY}"
  end
  if previous_timestamp && numeric.fetch("timestamp_unix") < previous_timestamp
    errors << "line #{line_number}: timestamp moved backwards"
  end
  if previous_elapsed && numeric.fetch("elapsed_seconds") < previous_elapsed
    errors << "line #{line_number}: elapsed time moved backwards"
  end
  previous_timestamp = numeric.fetch("timestamp_unix")
  previous_elapsed = numeric.fetch("elapsed_seconds")

  allowed_states = index == statuses.length - 1 ? ["COMPLETED"] : ["RUNNING"]
  unless allowed_states.include?(fields.fetch("state"))
    errors << "line #{line_number}: unexpected status state #{fields.fetch("state")}"
  end
  event[:numeric] = numeric
end

unless statuses.empty?
  final_status = statuses.last
  final_elapsed = final_status.fetch(:numeric, {}).fetch("elapsed_seconds", 0)
  if final_elapsed < EXPECTED_DURATION_SECONDS
    errors << "final status does not cover #{EXPECTED_DURATION_SECONDS} seconds"
  end

  rss_values = statuses.map { |event| event[:numeric]&.fetch("rss_bytes") }.compact
  if rss_values.length < RSS_WARM_UP_SAMPLES + RSS_GROWTH_WINDOW_SAMPLES
    errors << "receipt has too few RSS samples after warm-up"
  elsif rss_values[RSS_WARM_UP_SAMPLES..].each_cons(RSS_GROWTH_WINDOW_SAMPLES).any? { |window|
    window.each_cons(2).all? { |pair| pair[1] >= pair[0] } && window.last > window.first * 115 / 100
  }
    errors << "RSS has sustained monotonic growth greater than 15 percent"
  end
end

unless runners.empty?
  runner = runners.first.fetch(:fields)
  errors << "runner state is not COMPLETED" unless runner.fetch("runner_state") == "COMPLETED"
  errors << "runner mode is not --soak" unless runner.fetch("mode") == "--soak"
  unless runner.fetch("duration_seconds") == EXPECTED_DURATION_SECONDS.to_s
    errors << "runner duration is not #{EXPECTED_DURATION_SECONDS}"
  end
end

if errors.empty?
  batches = statuses.last.fetch(:numeric).fetch("batches")
  puts "p11-04 receipt: ok (#{statuses.length} status records, #{batches} batches)"
else
  warn errors.join("\n")
  exit 1
end
