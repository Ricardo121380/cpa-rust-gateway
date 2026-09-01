#!/usr/bin/env ruby
# frozen_string_literal: true

require "optparse"

ROOT = File.expand_path("..", __dir__)
DEFAULT_UNIT = File.join(ROOT, "deploy", "systemd", "cpa-rust-gateway.service")

options = { unit: DEFAULT_UNIT }
OptionParser.new do |parser|
  parser.on("--unit PATH", "Validate an alternate unit fixture") { |path| options[:unit] = path }
end.parse!

unless File.file?(options[:unit])
  warn "p12-02 systemd unit: unit file is unavailable"
  exit 1
end

lines = File.readlines(options[:unit], chomp: true)
required = [
  "[Unit]", "[Service]", "[Install]", "ConditionPathExists=/opt/cpa-rust-gateway/current/gateway",
  "User=cpa-gateway", "Group=cpa-gateway",
  "UMask=0077", "WorkingDirectory=/var/lib/cpa-rust-gateway",
  "ExecStart=/opt/cpa-rust-gateway/current/gateway serve --data-listen 127.0.0.1:18180 --management-listen 127.0.0.1:18181 --state-dir /var/lib/cpa-rust-gateway --credential-dir %d",
  "Restart=on-failure", "RestartSec=5s", "TimeoutStartSec=30s", "TimeoutStopSec=45s",
  "LimitNOFILE=65536", "MemoryMax=768M", "CPUQuota=200%", "TasksMax=512",
  "StateDirectory=cpa-rust-gateway", "StateDirectoryMode=0700",
  "RuntimeDirectory=cpa-rust-gateway", "RuntimeDirectoryMode=0700",
  "LogsDirectory=cpa-rust-gateway", "LogsDirectoryMode=0700",
  "LoadCredential=management-key:/etc/cpa-rust-gateway/credentials/management-key",
  "LoadCredential=management-csrf:/etc/cpa-rust-gateway/credentials/management-csrf",
  "LoadCredential=master-key:/etc/cpa-rust-gateway/credentials/master-key",
  "LoadCredential=backup-key:/etc/cpa-rust-gateway/credentials/backup-key",
  "LoadCredential=client-key-pepper:/etc/cpa-rust-gateway/credentials/client-key-pepper",
  "LoadCredential=grok-build-cache-key:/etc/cpa-rust-gateway/credentials/grok-build-cache-key",
  "NoNewPrivileges=yes", "CapabilityBoundingSet=", "AmbientCapabilities=", "PrivateTmp=yes",
  "PrivateDevices=yes", "ProtectSystem=strict", "ProtectHome=yes", "ProtectControlGroups=yes",
  "ProtectKernelTunables=yes", "ProtectKernelModules=yes", "ProtectKernelLogs=yes",
  "ProtectClock=yes", "ProtectHostname=yes", "ProtectProc=invisible", "ProcSubset=pid",
  "RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6", "RestrictNamespaces=yes",
  "RestrictRealtime=yes", "RestrictSUIDSGID=yes", "LockPersonality=yes",
  "MemoryDenyWriteExecute=yes", "SystemCallArchitectures=native",
  "SystemCallFilter=@system-service @network-io", "StandardOutput=journal",
  "StandardError=journal", "SyslogIdentifier=cpa-rust-gateway", "WantedBy=multi-user.target"
]

errors = required.reject { |directive| lines.include?(directive) }.map do |directive|
  "missing required directive: #{directive}"
end

exec_start = lines.grep(/^ExecStart=/)
errors << "unit must contain exactly one explicit serve ExecStart" unless exec_start.length == 1
errors << "unit must not expose a non-loopback listener" unless exec_start.all? do |line|
  line.include?("--data-listen 127.0.0.1:18180") && line.include?("--management-listen 127.0.0.1:18181")
end
errors << "unit must pass only systemd's credential directory" unless exec_start.all? { |line| line.end_with?("--credential-dir %d") }

forbidden = lines.grep(/^(Environment|EnvironmentFile)=/i)
errors << "unit must not use environment credential configuration" unless forbidden.empty?
errors << "unit must not use a root service account" if lines.include?("User=root") || lines.include?("Group=root")
errors << "unit must use all six direct LoadCredential entries" unless lines.grep(/^LoadCredential=/).length == 6
errors << "unit must not use unsupported ConditionPathIsExecutable" if lines.any? { |line| line.start_with?("ConditionPathIsExecutable=") }

if errors.any?
  errors.each { |error| warn "p12-02 systemd unit: #{error}" }
  exit 1
end

unless system("command -v systemd-analyze >/dev/null 2>&1")
  puts "p12-02 systemd unit: static invariants passed (systemd-analyze unavailable)"
  exit 0
end

# `systemd-analyze verify` resolves `ExecStart=` against the *live* filesystem, so verifying the
# repository unit verbatim can only pass on a host that already has the gateway installed -- it
# fails on every CI runner and every developer machine. The installed path is not what this step
# is for: the static table above already asserts the whole `ExecStart=` line byte-exact, so the
# path is fully covered without systemd. Point the analysed copy at an executable stub instead and
# let systemd check what only systemd can: that every directive parses and the unit is loadable.
# `LoadCredential=` sources are *not* resolved by verify (measured on systemd 255), so the six
# credential paths stay verbatim and keep failing closed at runtime rather than here.
#
# Exit status alone is nearly worthless here: verify exits 0 on an unknown key, an unknown section,
# an unparseable `Restart=` and an unparseable `ProtectSystem=`, and only exits non-zero when the
# unit is structurally unloadable. What it does do reliably is *print* one diagnostic naming the
# analysed file. So treat any such line as failure, which is what makes this step load-bearing.
require "open3"
require "tmpdir"

analysis_failed = Dir.mktmpdir("p12-02-systemd-unit") do |work_dir|
  stub = File.join(work_dir, "gateway")
  File.write(stub, "#!/bin/sh\n")
  File.chmod(0o755, stub)
  analysed = File.join(work_dir, File.basename(options[:unit]))
  File.write(analysed, File.read(options[:unit]).gsub("=/opt/cpa-rust-gateway/current/gateway", "=#{stub}"))

  output, status = Open3.capture2e("systemd-analyze", "verify", analysed)
  # Every diagnostic about the unit under test names its path; unrelated host units produce their
  # own lines (Ubuntu ships world-executable unit files, for instance) which must not fail this.
  diagnostics = output.lines.map(&:chomp).select { |line| line.include?(analysed) || line.include?(File.basename(analysed)) }
  if !status.success? || diagnostics.any?
    warn "p12-02 systemd unit: systemd-analyze verify rejected the unit"
    diagnostics.each { |line| warn "p12-02 systemd unit: #{line}" }
    warn "p12-02 systemd unit: exited #{status.exitstatus}" unless status.success?
    true
  else
    false
  end
end
exit 1 if analysis_failed

puts "p12-02 systemd unit: static invariants and systemd-analyze verify passed"
