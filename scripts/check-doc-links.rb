#!/usr/bin/env ruby

root = File.expand_path("..", __dir__)
files = [File.join(root, "README.md")] + Dir.glob(File.join(root, "docs", "**", "*.md")).sort
errors = []

files.each do |file|
  text = File.read(file, encoding: "UTF-8")
  text.scan(/\[[^\]]*\]\(([^)]+)\)/).flatten.each do |target|
    next if target.start_with?("http://", "https://", "mailto:", "#")

    path = target.split("#", 2).first
    next if path.nil? || path.empty?

    resolved = File.expand_path(path, File.dirname(file))
    errors << "#{file.delete_prefix(root + "/")}: missing #{target}" unless File.exist?(resolved)
  end
end

if errors.empty?
  puts "doc-links: ok (#{files.length} Markdown files)"
else
  warn errors.join("\n")
  exit 1
end
