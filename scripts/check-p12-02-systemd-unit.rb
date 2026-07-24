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
  "[Unit]", "[Service]", "[Install]", "User=cpa-gateway", "Group=cpa-gateway",
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
errors << "unit must use all five direct LoadCredential entries" unless lines.grep(/^LoadCredential=/).length == 5

if errors.any?
  errors.each { |error| warn "p12-02 systemd unit: #{error}" }
  exit 1
end

if system("command -v systemd-analyze >/dev/null 2>&1")
  unless system("systemd-analyze", "verify", options[:unit])
    warn "p12-02 systemd unit: systemd-analyze verify failed"
    exit 1
  end
  puts "p12-02 systemd unit: static invariants and systemd-analyze verify passed"
else
  puts "p12-02 systemd unit: static invariants passed (systemd-analyze unavailable)"
end
