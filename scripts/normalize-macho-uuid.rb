#!/usr/bin/env ruby

require "digest"
require "open3"

LC_UUID = 0x1b
MACHO_64_LE = "\xCF\xFA\xED\xFE".b
UUID_SIZE = 16

path = ARGV.fetch(0) do
  warn "usage: #{File.basename($PROGRAM_NAME)} PATH"
  exit 2
end

abort "macho-uuid: missing artifact #{path}" unless File.file?(path)

data = File.binread(path)
unless data.start_with?(MACHO_64_LE)
  puts "macho-uuid: skipped non-Mach-O artifact #{path}"
  exit 0
end

codesign = "/usr/bin/codesign"
abort "macho-uuid: codesign is unavailable" unless File.executable?(codesign)

identifier = "dev.cpa-rust-gateway.gateway"
signature_out, signature_err, signature_status = Open3.capture3(
  codesign,
  "--display",
  "--verbose=2",
  path
)
signature_details = signature_out + signature_err
verified = system(codesign, "--verify", "--strict", path, out: File::NULL, err: File::NULL)
if signature_status.success? && verified && signature_details.include?("Identifier=#{identifier}")
  puts "macho-uuid: already normalized and signed as #{identifier}"
  exit 0
end

if signature_status.success?
  removed = system(codesign, "--remove-signature", path, out: File::NULL, err: File::NULL)
  abort "macho-uuid: could not remove existing signature" unless removed
  data = File.binread(path)
end

abort "macho-uuid: truncated Mach-O header" if data.bytesize < 32

ncmds = data.byteslice(16, 4).unpack1("V")
offset = 32
uuid_offsets = []

ncmds.times do
  abort "macho-uuid: truncated load command" if offset + 8 > data.bytesize

  command, command_size = data.byteslice(offset, 8).unpack("V2")
  if command_size < 8 || offset + command_size > data.bytesize
    abort "macho-uuid: invalid load command size #{command_size}"
  end

  if command == LC_UUID
    abort "macho-uuid: invalid LC_UUID size #{command_size}" unless command_size == 24
    uuid_offsets << offset + 8
  end

  offset += command_size
end

abort "macho-uuid: expected one LC_UUID, found #{uuid_offsets.length}" unless uuid_offsets.length == 1

uuid_offset = uuid_offsets.first
canonical = data.dup
canonical[uuid_offset, UUID_SIZE] = "\0" * UUID_SIZE
uuid_bytes = Digest::SHA256.digest(canonical).bytes.first(UUID_SIZE)
uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50
uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80
uuid = uuid_bytes.pack("C*")

File.open(path, "r+b") do |artifact|
  artifact.seek(uuid_offset)
  artifact.write(uuid)
end

signed = system(
  codesign,
  "--force",
  "--sign",
  "-",
  "--timestamp=none",
  "--identifier",
  identifier,
  path,
  out: File::NULL,
  err: File::NULL
)
abort "macho-uuid: deterministic ad-hoc signing failed" unless signed

verified = system(codesign, "--verify", "--strict", path, out: File::NULL, err: File::NULL)
abort "macho-uuid: signature verification failed" unless verified

hex = uuid.unpack1("H*").upcase
formatted = [hex[0, 8], hex[8, 4], hex[12, 4], hex[16, 4], hex[20, 12]].join("-")
puts "macho-uuid: normalized #{formatted} and signed as #{identifier}"
