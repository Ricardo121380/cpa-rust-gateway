#!/usr/bin/env ruby

# Verifies that every test function a behavior contract names actually exists.
#
# Each contract's "Corresponding tests" section is the documented evidence chain from a required
# behavior to the test that proves it. Nothing validated those names, so a renamed or deleted test
# left the contract asserting evidence that no longer exists — the chain silently rots exactly
# where an auditor or a phase gate would rely on it.

require "set"

root = File.expand_path("..", __dir__)
contracts_dir = File.join(root, "docs", "contracts")
abort("contract-tests: missing #{contracts_dir}") unless Dir.exist?(contracts_dir)

# One identifier per line item under a "Corresponding tests" heading, e.g. "- `some_test_name`".
TEST_NAME = /\A[a-z_][a-z0-9_]*\z/

def rust_sources(root)
  Dir.glob(File.join(root, "{apps,crates,tests}", "**", "*.rs"))
end

defined_tests = Set.new
rust_sources(root).each do |path|
  File.foreach(path, encoding: "UTF-8") do |line|
    match = line.match(/\bfn\s+([a-z_][a-z0-9_]*)\s*[(<]/)
    defined_tests << match[1] if match
  end
end

errors = []
checked = 0

Dir.glob(File.join(contracts_dir, "*.md")).sort.each do |path|
  relative = path.delete_prefix("#{root}/")
  in_section = false

  File.foreach(path, encoding: "UTF-8").with_index(1) do |line, number|
    if line.start_with?("#")
      in_section = line.match?(/\A#+\s*Corresponding tests\s*\z/)
      next
    end
    next unless in_section

    # Only bare single-identifier bullets are treated as test references; prose bullets that
    # merely mention a suite in backticks are not claims about one function.
    match = line.match(/\A[-*]\s+`([^`]+)`\s*\z/)
    next unless match

    name = match[1]
    next unless name.match?(TEST_NAME)

    checked += 1
    unless defined_tests.include?(name)
      errors << "#{relative}:#{number}: names a test that does not exist: #{name}"
    end
  end
end

if errors.empty?
  puts "contract-tests: ok (#{checked} referenced tests resolved)"
else
  warn errors.join("\n")
  warn "contract-tests: #{errors.length} of #{checked} referenced tests are missing"
  exit 1
end
