#!/usr/bin/env ruby

root = File.expand_path("..", __dir__)
source_files = Dir.glob(File.join(root, "{apps,crates,tests}", "**", "*.rs")).sort
crate_roots = Dir.glob(File.join(root, "apps", "*", "src", "main.rs")).sort +
              Dir.glob(File.join(root, "crates", "*", "src", "lib.rs")).sort +
              Dir.glob(File.join(root, "tests", "*", "src", "lib.rs")).sort
errors = []

source_files.each do |path|
  relative = path.delete_prefix(root + "/")
  text = File.read(path, encoding: "UTF-8")
  errors << "#{relative}: invalid UTF-8" unless text.valid_encoding?
  errors << "#{relative}: crate-level unsafe allow" if text.match?(/#!\s*\[\s*allow\s*\(\s*unsafe_code\s*\)\s*\]/)
  errors << "#{relative}: unwrap() is forbidden" if text.match?(/\.unwrap\s*\(/)
  errors << "#{relative}: expect() is forbidden" if text.match?(/\.expect\s*\(/)
  errors << "#{relative}: panic!() is forbidden" if text.match?(/\bpanic!\s*\(/)
  errors << "#{relative}: TODO/FIXME requires a tracked task" if text.match?(/\b(?:TODO|FIXME)\b/)
end

crate_roots.each do |path|
  relative = path.delete_prefix(root + "/")
  text = File.read(path, encoding: "UTF-8")
  errors << "#{relative}: missing #![deny(unsafe_code)]" unless text.include?("#![deny(unsafe_code)]")
end

if source_files.empty? || crate_roots.empty?
  errors << "no Rust source or crate roots discovered"
end

if errors.empty?
  puts "source-policy: ok (#{source_files.length} Rust files, #{crate_roots.length} crate roots)"
else
  warn errors.join("\n")
  exit 1
end
