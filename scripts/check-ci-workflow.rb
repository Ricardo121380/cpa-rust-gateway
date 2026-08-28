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

# Fast compiles every workspace target and then links the large gateway test/bin graph. GitHub's
# clean runner does not have enough ephemeral disk for the default dev/test debug information.
# These profile overrides remove only debug symbols; they must not weaken features, Clippy or test
# coverage. Guard them here so a workflow-only edit cannot reintroduce the disk exhaustion.
delivery_fast_env = delivery_jobs.dig("fast", "env")
unless delivery_fast_env.is_a?(Hash)
  errors << "delivery fast job must define disk-bounded Cargo profile overrides"
  delivery_fast_env = {}
end
assert_equal(errors, "delivery fast dev debug", "0", delivery_fast_env["CARGO_PROFILE_DEV_DEBUG"])
assert_equal(errors, "delivery fast test debug", "0", delivery_fast_env["CARGO_PROFILE_TEST_DEBUG"])

# `gateway-http-actix` runs the management SPA build from its Cargo build script.  Keep the
# lightweight code path usable on a fresh runner by pinning the JavaScript toolchain, restoring
# only npm's lockfile-scoped cache, and installing the SPA before any Cargo command can invoke the
# build script.  These checks intentionally inspect the parsed step structure as well as the exact
# install command so a renamed or reordered step cannot silently reintroduce the cold-run failure.
code_steps = ci_jobs.dig("code", "steps")
unless code_steps.is_a?(Array)
  errors << "ci code job must define a steps array"
  code_steps = []
end

code_env = ci_jobs.dig("code", "env")
node_version = code_env.is_a?(Hash) ? code_env["NODE_VERSION"] : nil
npm_version = code_env.is_a?(Hash) ? code_env["NPM_VERSION"] : nil
errors << "ci code job must pin NODE_VERSION to a full semver" unless node_version.is_a?(String) && node_version.match?(/\A\d+\.\d+\.\d+\z/)
errors << "ci code job must pin NPM_VERSION to a full semver" unless npm_version.is_a?(String) && npm_version.match?(/\A\d+\.\d+\.\d+\z/)

setup_node_steps = code_steps.select do |step|
  step.is_a?(Hash) && step["uses"].is_a?(String) && step["uses"].start_with?("actions/setup-node@")
end
setup_node_index = code_steps.index do |step|
  step.is_a?(Hash) && step["uses"].is_a?(String) && step["uses"].start_with?("actions/setup-node@")
end
if setup_node_steps.length != 1
  errors << "ci code job must have exactly one actions/setup-node step"
else
  setup_node = setup_node_steps.fetch(0)
  setup_with = setup_node["with"]
  unless setup_with.is_a?(Hash)
    errors << "ci setup-node step must define with inputs"
    setup_with = {}
  end
  assert_equal(errors, "ci setup-node node-version", "${{ env.NODE_VERSION }}", setup_with["node-version"])
  assert_equal(errors, "ci setup-node cache", "npm", setup_with["cache"])
  assert_equal(errors, "ci setup-node cache dependency path", "web/admin-ui/package-lock.json", setup_with["cache-dependency-path"])
  errors << "ci setup-node must not resolve a moving latest version" if setup_with["check-latest"] == true
end

version_check_index = code_steps.index do |step|
  run = step.is_a?(Hash) ? step["run"] : nil
  run.is_a?(String) && run.include?("node --version") && run.include?("npm --version")
end
if version_check_index.nil?
  errors << "ci code job must verify the pinned Node.js and npm versions"
else
  version_check_run = code_steps.fetch(version_check_index).fetch("run")
  unless version_check_run.include?("v${NODE_VERSION}") && version_check_run.include?("$NPM_VERSION")
    errors << "ci code job version check must compare against NODE_VERSION and NPM_VERSION"
  end
end

npm_install_index = code_steps.index do |step|
  next false unless step.is_a?(Hash) && step["run"].is_a?(String)

  step["run"].include?("npm ci --ignore-scripts --no-audit --no-fund")
end
if npm_install_index.nil?
  errors << "ci code job must install the management SPA with locked npm ci"
else
  npm_step = code_steps.fetch(npm_install_index)
  assert_equal(errors, "ci npm install working-directory", "web/admin-ui", npm_step["working-directory"])
end

if setup_node_index && npm_install_index && setup_node_index >= npm_install_index
  errors << "ci setup-node must precede npm install"
end
if setup_node_index && version_check_index && setup_node_index >= version_check_index
  errors << "ci setup-node must precede its Node.js/npm version check"
end

cargo_indices = code_steps.each_index.select do |index|
  run = code_steps.fetch(index).is_a?(Hash) ? code_steps.fetch(index)["run"] : nil
  run.is_a?(String) && run.match?(/\bcargo(?:\s|\+)/)
end
if npm_install_index && !cargo_indices.empty? && npm_install_index >= cargo_indices.min
  errors << "ci npm install must precede every Cargo command in the code job"
end

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
  text.scan(/^\s*uses:\s*([^\s#]+)/).flatten.each do |action|
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
