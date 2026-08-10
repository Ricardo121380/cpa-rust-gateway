#!/usr/bin/env ruby
# frozen_string_literal: true

# Validate the two delivery boundaries without relying on a YAML 1.1 parser's treatment of the
# `on` key (Psych exposes it as `true`).  The ordinary `ci` workflow is intentionally lightweight;
# the separate `delivery-gate` workflow is the only place where Fast and supply-chain checks may
# run.

require "psych"

ROOT = File.expand_path("..", __dir__)
WORKFLOW_DIR = File.join(ROOT, ".github", "workflows")
errors = []

def load_workflow(path, errors)
  document = Psych.safe_load(File.read(path, encoding: "UTF-8"), aliases: true)
  unless document.is_a?(Hash)
    errors << "workflow is not a mapping: #{path}"
    return {}
  end
  document
rescue Psych::SyntaxError => error
  errors << "invalid YAML in #{path}: #{error.message}"
  {}
end

def trigger(document)
  # YAML 1.1 treats the unquoted `on` key as boolean true. GitHub itself uses the string key.
  document["on"] || document[true] || {}
end

def assert_equal(errors, label, expected, observed)
  errors << "#{label}: expected #{expected.inspect}, got #{observed.inspect}" unless observed == expected
end

ci_path = File.join(WORKFLOW_DIR, "ci.yml")
delivery_path = File.join(WORKFLOW_DIR, "delivery-gate.yml")
ci = load_workflow(ci_path, errors)
delivery = load_workflow(delivery_path, errors)

assert_equal(errors, "ci name", "ci", ci["name"])
assert_equal(errors, "delivery-gate name", "delivery-gate", delivery["name"])

ci_on = trigger(ci)
delivery_on = trigger(delivery)
assert_equal(errors, "ci push branches", ["main"], ci_on.dig("push", "branches"))
errors << "ci must not trigger on tags" if ci_on.dig("push", "tags")
errors << "ci must not expose workflow_dispatch" if ci_on.key?("workflow_dispatch")
assert_equal(errors, "ci pull_request types", ["opened", "synchronize", "reopened"], ci_on.dig("pull_request", "types"))

assert_equal(errors, "delivery push tags", ["phase-p*-complete"], delivery_on.dig("push", "tags"))
errors << "delivery-gate must not trigger on ordinary branch pushes" if delivery_on.dig("push", "branches")
errors << "delivery-gate must expose workflow_dispatch" unless delivery_on.key?("workflow_dispatch")
assert_equal(errors, "delivery pull_request types", ["labeled", "synchronize"], delivery_on.dig("pull_request", "types"))
closeout_input = delivery_on.dig("workflow_dispatch", "inputs", "closeout_reason")
unless closeout_input.is_a?(Hash) && closeout_input["required"] == true
  errors << "delivery workflow_dispatch must require closeout_reason"
end

ci_jobs = ci.fetch("jobs", {})
delivery_jobs = delivery.fetch("jobs", {})
%w[classify docs code required].each do |job|
  errors << "ci missing lightweight job: #{job}" unless ci_jobs.key?(job)
end
%w[authorize fast full required].each do |job|
  errors << "delivery-gate missing formal job: #{job}" unless delivery_jobs.key?(job)
end

assert_equal(errors, "ci required job name", "Required lightweight gate", ci_jobs.dig("required", "name"))
assert_equal(errors, "delivery required job name", "Required delivery gate", delivery_jobs.dig("required", "name"))

ci_text = File.read(ci_path, encoding: "UTF-8")
delivery_text = File.read(delivery_path, encoding: "UTF-8")

# A lightweight workflow must never accidentally reintroduce a formal command. This is a direct
# textual guard in addition to the structural trigger checks above, so a renamed step cannot hide
# an expensive gate behind a harmless-looking job name.
[
  "./scripts/check.sh fast",
  "./scripts/check.sh full",
  "./scripts/check.sh supply-chain",
  "cargo deny",
  "cargo audit"
].each do |forbidden|
  errors << "ci contains formal delivery command: #{forbidden}" if ci_text.include?(forbidden)
end
[
  "delivery-closeout",
  "github.event.pull_request.head.sha",
  "github.sha",
  "needs.authorize.outputs.revision",
  "cancel-in-progress: ${{ github.event_name == 'pull_request' }}",
  "./scripts/check.sh fast",
  "./scripts/check.sh supply-chain"
].each do |required|
  errors << "delivery-gate missing required boundary fragment: #{required}" unless delivery_text.include?(required)
end
errors << "delivery-gate must state exact-head invalidation" unless delivery_text.include?("remove and re-add the label")

# Every third-party action in both workflows must be immutable. This catches new actions in either
# file, while allowing ordinary `run:` steps to evolve independently.
[ci_path, delivery_path].each do |path|
  text = File.read(path, encoding: "UTF-8")
  text.scan(/^\s*-\s+uses:\s*([^\s#]+)/).flatten.each do |action|
    reference = action.split("@", 2)[1]
    unless reference && reference.match?(/\A[0-9a-f]{40}\z/)
      errors << "GitHub Action is not pinned to a full commit SHA: #{action} (#{path})"
    end
  end
end

if errors.empty?
  puts "ci-workflows: ok (lightweight PR boundary, explicit formal delivery boundary, pinned actions)"
else
  warn errors.join("\n")
  exit 1
end
