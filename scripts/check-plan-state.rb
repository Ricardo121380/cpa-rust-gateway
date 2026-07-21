#!/usr/bin/env ruby

require "set"

root = File.expand_path("..", __dir__)
path = File.join(root, "docs", "06-development-plan.md")

if ARGV.first == "--plan"
  ARGV.shift
  path = ARGV.shift || abort("plan-state: --plan requires a path")
end
abort("plan-state: unexpected arguments") unless ARGV.empty?

statuses = Set.new(%w[PENDING IN_PROGRESS LOCAL_PASS_PENDING_CI DONE BLOCKED DEFERRED])
tasks = {}

File.foreach(path, encoding: "UTF-8") do |line|
  columns = line.split("|").map(&:strip)
  next unless columns.length == 7

  task_id = columns[1]
  next unless task_id.match?(/\AP\d{1,2}-\d{2}\z/)

  tasks[task_id] = { dependencies: columns[3], status: columns[5] }
end

errors = []
tasks.each do |task_id, task|
  errors << "#{task_id} has an unknown status #{task[:status]}" unless statuses.include?(task[:status])
end

in_progress = tasks.select { |_task_id, task| task[:status] == "IN_PROGRESS" }.keys
if in_progress.length > 1
  errors << "more than one IN_PROGRESS task: #{in_progress.join(', ')}"
end

active_statuses = Set.new(%w[IN_PROGRESS LOCAL_PASS_PENDING_CI DONE])
tasks.each do |task_id, task|
  next unless active_statuses.include?(task[:status])

  task[:dependencies].scan(/\bP\d{1,2}-\d{2}\b/).each do |dependency|
    next unless tasks.key?(dependency)
    next if tasks.fetch(dependency).fetch(:status) == "DONE"

    errors << "#{task_id} is #{task[:status]} before dependency #{dependency} is DONE"
  end
end

p4_00 = tasks["P4-00"]
if p4_00 && p4_00[:status] != "DONE"
  tasks.each do |task_id, task|
    next unless task_id.match?(/\AP4-0[1-9]\z/)
    next unless active_statuses.include?(task[:status])

    errors << "#{task_id} is #{task[:status]} before P4-00 is DONE"
  end
end

if errors.empty?
  puts "plan-state: ok (#{tasks.length} tasks, #{in_progress.length} IN_PROGRESS)"
else
  warn errors.join("\n")
  exit 1
end
