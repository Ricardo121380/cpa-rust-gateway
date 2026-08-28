#!/usr/bin/env ruby

root = File.expand_path("..", __dir__)
plan = File.read(File.join(root, "docs", "06-development-plan.md"), encoding: "UTF-8")
readme = File.read(File.join(root, "README.md"), encoding: "UTF-8")
traceability = File.read(File.join(root, "docs", "traceability.md"), encoding: "UTF-8")
root_manifest = File.read(File.join(root, "Cargo.toml"), encoding: "UTF-8")
deny_config = File.read(File.join(root, "deny.toml"), encoding: "UTF-8")
errors = []

expected_tasks = (1..6).map { |number| format("P0-%02d", number) }
task_statuses = {}
plan.each_line do |line|
  next unless line =~ /^\| (P0-\d{2}) \|/
  parts = line.split("|").map(&:strip)
  task_statuses[parts[1]] = parts[5]
end

errors << "P0 task IDs differ: #{task_statuses.keys.inspect}" unless task_statuses.keys == expected_tasks
task_statuses.each do |task, status|
  errors << "#{task} is #{status}, expected DONE" unless status == "DONE"
end

errors << "P0 phase is not DONE" unless plan.match?(/^\| P0 \|.*\| DONE \|$/)
errors << "README does not declare G0 passed" unless readme.include?("G0 已通过")
errors << "traceability does not mark G0 DONE" unless traceability.match?(/^\| G0 \|.*\| DONE \|$/)
errors << "workspace unsafe_code is not deny" unless root_manifest.include?('unsafe_code = "deny"')

%w[AGPL GPL SSPL].each do |license_family|
  errors << "deny.toml allowlist contains #{license_family}" if deny_config.match?(/^\s*"[^"]*#{license_family}/)
end

required_reports = %w[
  p0-01-repository-baseline.md
  p0-02-document-traceability.md
  p0-03-rust-workspace.md
  p0-04-quality-gates.md
  p0-05-ci-baseline.md
  p0-05-clean-checkout-log.md
  environment-baseline.md
  g0-gate-report.md
  g0-reproducible-build-log.md
]

required_reports.each do |report|
  path = File.join(root, "docs", "reports", report)
  errors << "missing report #{report}" unless File.file?(path)
end

unsafe_adrs = Dir.glob(File.join(root, "docs", "adr", "ADR-*.md")).select do |path|
  text = File.read(path, encoding: "UTF-8")
  text.match?(/unsafe/i) && text.match?(/Status:\s*Accepted/i)
end
errors << "accepted unsafe exception ADR exists: #{unsafe_adrs.join(', ')}" unless unsafe_adrs.empty?

if errors.empty?
  puts "p0-gate-state: ok (6 tasks DONE, phase DONE, reports and policies present)"
else
  warn errors.join("\n")
  exit 1
end
