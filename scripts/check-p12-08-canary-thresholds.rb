#!/usr/bin/env ruby
# frozen_string_literal: true

# The direct-cutover observation floor is checked rather than asserted. This fails closed if the
# 72-hour/1250-success rule, no-split topology, or G12 severity taxonomy disappears.

ROOT = File.expand_path("..", __dir__)
PLAN = File.join(ROOT, "docs", "06-development-plan.md")

REQUIRED_SUCCESSES = 1250
REJECTED_FLOOR = 100
TRIGGER_DELTA = 0.01
ASSUMED_BASELINE = 0.005
CONFIDENCE = 0.95
ALPHA_Z = 1.645 # one-sided alpha = 0.05
BETA_Z = 0.842  # power = 0.80

errors = []

# With zero observed failures the true rate is still only bounded above by 1 - (1-c)^(1/n).
def zero_failure_upper_bound(sample, confidence)
  1.0 - ((1.0 - confidence)**(1.0 / sample))
end

# Per-arm size needed to separate two proportions at the given alpha and power.
def per_arm_sample(baseline, delta)
  other = baseline + delta
  spread = (baseline * (1 - baseline)) + (other * (1 - other))
  (((ALPHA_Z + BETA_Z)**2 * spread) / (delta**2)).ceil
end

needed = per_arm_sample(ASSUMED_BASELINE, TRIGGER_DELTA)
if needed > REQUIRED_SUCCESSES
  errors << "the #{REQUIRED_SUCCESSES}-success floor no longer detects a #{TRIGGER_DELTA * 100}pp " \
            "increase at a #{ASSUMED_BASELINE * 100}% baseline (needs #{needed})"
end
if per_arm_sample(ASSUMED_BASELINE, TRIGGER_DELTA) <= REJECTED_FLOOR
  errors << "the rejected #{REJECTED_FLOOR}-success floor would now suffice, so the rationale is stale"
end

rejected_bound = zero_failure_upper_bound(REJECTED_FLOOR, CONFIDENCE)
if rejected_bound <= TRIGGER_DELTA
  errors << "a zero-failure run of #{REJECTED_FLOOR} would already exclude the #{TRIGGER_DELTA * 100}pp trigger"
end
accepted_bound = zero_failure_upper_bound(REQUIRED_SUCCESSES, CONFIDENCE)
if accepted_bound > TRIGGER_DELTA
  errors << "a zero-failure run of #{REQUIRED_SUCCESSES} still cannot exclude the trigger " \
            "(bound #{(accepted_bound * 100).round(2)}%)"
end

begin
  plan = File.read(PLAN, encoding: "UTF-8")
rescue Errno::ENOENT
  warn "p12-08-canary: development plan is missing"
  exit 1
end

required_fragments = [
  "不执行 10%→25%→50%→100% 百分比或按 Key 分流",
  "切换后 CPAR 全量观察至少 72h，成功请求不少于 1250",
  "合成补足请求单独计数",
  "G12 通过后停止并禁用旧 CPA",
  "### 故障严重度分级",
  "无 P0/P1 故障（按上表分级判定）",
  "#### 判据信号的实际来源"
]
required_fragments.each do |fragment|
  errors << "plan no longer records the Canary/G12 decision: #{fragment}" unless plan.include?(fragment)
end

# The signal-availability table is the honest half of this decision: it stops a future reader from
# assuming the gateway serves latency percentiles it does not. Both the prose and the table row must
# keep saying so, and the exposition must actually still lack a histogram.
[
  "TTFT 与 P95/P99 当前服务端不可观测",
  "| TTFT | 客户端侧合成探针记录首字节时长 | 服务端不可观测 |",
  "| P95/P99 | 同上；Attempt 级时长由事件日志 `started_at_ms`/`ended_at_ms` 离线导出统计 | 服务端无实时 histogram |"
].each do |fragment|
  errors << "plan no longer records the latency observability gap: #{fragment}" unless plan.include?(fragment)
end

telemetry_path = File.join(ROOT, "crates", "gateway-observability", "src", "telemetry.rs")
begin
  telemetry = File.read(telemetry_path, encoding: "UTF-8")
  if telemetry.match?(/histogram|_bucket/i)
    errors << "the Prometheus exposition now has a histogram, so the recorded latency gap is stale " \
              "and the Canary latency evidence path should be revisited"
  end
  # The severity taxonomy names specific label values as its mechanical P1 signals. A renamed or
  # removed label would leave an operator grepping for something that is never exported.
  %w[required_quarantined write_failed required_queue_full sink_closed].each do |label|
    unless telemetry.include?("\"#{label}\"")
      errors << "the taxonomy's P1 signal #{label} is no longer exported by the telemetry exposition"
    end
    unless plan.include?("outcome=#{label}")
      errors << "the severity taxonomy no longer names the #{label} signal"
    end
  end
rescue Errno::ENOENT
  errors << "telemetry exposition is missing"
end

# Each severity level must stay defined, or "no P0/P1" becomes unjudgeable again.
%w[P0 P1 P2 P3].each do |level|
  unless plan.match?(/^\| #{level} \| /)
    errors << "severity taxonomy is missing a #{level} row"
  end
end

# The floor must not silently revert to a number the statistics reject. The rejected value is still
# quoted inside the CR that argued against it, so only a live rule counts as a regression.
live_rules = plan.lines.reject { |line| line.start_with?("      ") }.join
if live_rules.match?(/观察窗口.*至少\s*#{REJECTED_FLOOR}\s*个成功请求/)
  errors << "plan still imposes the rejected #{REJECTED_FLOOR}-success floor as a live rule"
end

if errors.empty?
  puts "p12-08-canary: ok (direct cutover; 72h/#{REQUIRED_SUCCESSES}-success observation; " \
       "no production split; severity taxonomy present)"
else
  warn errors.join("\n")
  exit 1
end
