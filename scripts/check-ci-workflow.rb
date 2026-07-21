#!/usr/bin/env ruby

require "psych"

root = File.expand_path("..", __dir__)
path = File.join(root, ".github", "workflows", "ci.yml")
text = File.read(path, encoding: "UTF-8")
errors = []

begin
  Psych.parse(text)
rescue Psych::SyntaxError => error
  errors << "invalid YAML: #{error.message}"
end

required_fragments = [
  "branches:",
  "- main",
  "tags:",
  '- "phase-p*-complete"',
  "jobs:",
  "classify:",
  "docs:",
  "fast:",
  "full:",
  "required:",
  "Required delivery gate",
  "needs: [classify, fast]",
  "needs: classify",
  "./scripts/classify-ci-change.sh",
  "./scripts/check.sh docs",
  "./scripts/check.sh fast",
  "./scripts/check.sh supply-chain",
  "./scripts/install-quality-tools.sh",
  "actions/cache@5a3ec84eff668545956fd18022155c47e93e2684",
  "id: quality-tool-cache",
  "GITHUB_STEP_SUMMARY",
  "tools/quality-tool-versions.env",
  "rustup toolchain install 1.97.1",
]

required_fragments.each do |fragment|
  errors << "missing required workflow fragment: #{fragment}" unless text.include?(fragment)
end

expected_push_trigger = <<~YAML
  on:
    push:
      branches:
        - main
      tags:
        - "phase-p*-complete"
    pull_request:
    workflow_dispatch:
YAML
unless text.include?(expected_push_trigger)
  errors << "push trigger must be limited to main and phase completion tags"
end

text.scan(/^\s*-\s+uses:\s*([^\s#]+)/).flatten.each do |action|
  reference = action.split("@", 2)[1]
  unless reference && reference.match?(/\A[0-9a-f]{40}\z/)
    errors << "GitHub Action is not pinned to a full commit SHA: #{action}"
  end
end

if errors.empty?
  puts "ci-workflow: ok (syntax, required jobs, pinned actions)"
else
  warn errors.join("\n")
  exit 1
end
