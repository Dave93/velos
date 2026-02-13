const std = @import("std");

// Wire format:
// Header: [0xVE, 0x10, version(1), length_le(4)] = 7 bytes
// Payload: Request or Response in simple binary encoding

pub const MAGIC_0: u8 = 0x56; // ASCII 'V'
pub const MAGIC_1: u8 = 0x10;
pub const PROTOCOL_VERSION: u8 = 0x01;
pub const HEADER_SIZE: usize = 7;
pub const MAX_PAYLOAD_SIZE: u32 = 4 * 1024 * 1024; // 4 MB max

pub const Command = enum(u8) {
    process_start = 0x01,
    process_stop = 0x02,
    process_restart = 0x03,
    process_delete = 0x04,
    process_list = 0x05,
    process_info = 0x06,
    process_scale = 0x07,
    log_read = 0x10,
    state_save = 0x30,
    state_load = 0x31,
    ping = 0x40,
    shutdown = 0x41,
    _,
};

pub const Status = enum(u8) {
    ok = 0,
    err = 1,
    _,
};

pub const Request = struct {
    id: u32,
    command: Command,
    payload: []const u8, // raw payload bytes, interpretation depends on command
};

pub const Response = struct {
    id: u32,
    status: Status,
    payload: []const u8, // raw payload bytes
};

/// Encode a 7-byte header into buf. Returns the header slice.
pub fn encodeHeader(buf: *[HEADER_SIZE]u8, payload_len: u32) void {
    buf[0] = MAGIC_0;
    buf[1] = MAGIC_1;
    buf[2] = PROTOCOL_VERSION;
    std.mem.writeInt(u32, buf[3..7], payload_len, .little);
}

/// Validate a 7-byte header. Returns payload length or error.
pub fn decodeHeader(buf: *const [HEADER_SIZE]u8) !u32 {
    if (buf[0] != MAGIC_0 or buf[1] != MAGIC_1) return error.InvalidMagic;
    if (buf[2] != PROTOCOL_VERSION) return error.UnsupportedVersion;
    const len = std.mem.readInt(u32, buf[3..7], .little);
    if (len > MAX_PAYLOAD_SIZE) return error.PayloadTooLarge;
    return len;
}

/// Encode a Request into binary: id(4) + command(1) + payload
pub fn encodeRequest(allocator: std.mem.Allocator, req: Request) ![]u8 {
    const total = 5 + req.payload.len; // 4 bytes id + 1 byte command + payload
    const buf = try allocator.alloc(u8, total);
    std.mem.writeInt(u32, buf[0..4], req.id, .little);
    buf[4] = @intFromEnum(req.command);
    if (req.payload.len > 0) {
        @memcpy(buf[5..], req.payload);
    }
    return buf;
}

/// Decode a Request from binary
pub fn decodeRequest(data: []const u8) !Request {
    if (data.len < 5) return error.TruncatedRequest;
    const id = std.mem.readInt(u32, data[0..4], .little);
    const cmd: Command = @enumFromInt(data[4]);
    return Request{
        .id = id,
        .command = cmd,
        .payload = data[5..],
    };
}

/// Encode a Response into binary: id(4) + status(1) + payload
pub fn encodeResponse(allocator: std.mem.Allocator, resp: Response) ![]u8 {
    const total = 5 + resp.payload.len;
    const buf = try allocator.alloc(u8, total);
    std.mem.writeInt(u32, buf[0..4], resp.id, .little);
    buf[4] = @intFromEnum(resp.status);
    if (resp.payload.len > 0) {
        @memcpy(buf[5..], resp.payload);
    }
    return buf;
}

/// Decode a Response from binary
pub fn decodeResponse(data: []const u8) !Response {
    if (data.len < 5) return error.TruncatedResponse;
    const id = std.mem.readInt(u32, data[0..4], .little);
    const status: Status = @enumFromInt(data[4]);
    return Response{
        .id = id,
        .status = status,
        .payload = data[5..],
    };
}

// === Payload encoding helpers ===
// Simple binary format: LE integers, length-prefixed strings (u32 len + bytes)

pub fn writeU32(buf: []u8, offset: usize, val: u32) usize {
    std.mem.writeInt(u32, buf[offset..][0..4], val, .little);
    return offset + 4;
}

pub fn readU32(buf: []const u8, offset: usize) struct { val: u32, next: usize } {
    if (offset + 4 > buf.len) return .{ .val = 0, .next = offset };
    const val = std.mem.readInt(u32, buf[offset..][0..4], .little);
    return .{ .val = val, .next = offset + 4 };
}

pub fn writeU64(buf: []u8, offset: usize, val: u64) usize {
    std.mem.writeInt(u64, buf[offset..][0..8], val, .little);
    return offset + 8;
}

pub fn readU64(buf: []const u8, offset: usize) struct { val: u64, next: usize } {
    if (offset + 8 > buf.len) return .{ .val = 0, .next = offset };
    const val = std.mem.readInt(u64, buf[offset..][0..8], .little);
    return .{ .val = val, .next = offset + 8 };
}

pub fn writeString(buf: []u8, offset: usize, s: []const u8) usize {
    const off = writeU32(buf, offset, @intCast(s.len));
    @memcpy(buf[off..][0..s.len], s);
    return off + s.len;
}

pub fn readString(buf: []const u8, offset: usize) struct { val: []const u8, next: usize } {
    const len_res = readU32(buf, offset);
    const len = len_res.val;
    const start = len_res.next;
    if (start + len > buf.len) return .{ .val = &[_]u8{}, .next = start };
    return .{ .val = buf[start..][0..len], .next = start + len };
}

pub fn writeU8(buf: []u8, offset: usize, val: u8) usize {
    buf[offset] = val;
    return offset + 1;
}

pub fn readU8(buf: []const u8, offset: usize) struct { val: u8, next: usize } {
    if (offset >= buf.len) return .{ .val = 0, .next = offset };
    return .{ .val = buf[offset], .next = offset + 1 };
}

pub fn writeI32(buf: []u8, offset: usize, val: i32) usize {
    std.mem.writeInt(i32, buf[offset..][0..4], val, .little);
    return offset + 4;
}

pub fn readI32(buf: []const u8, offset: usize) struct { val: i32, next: usize } {
    if (offset + 4 > buf.len) return .{ .val = 0, .next = offset };
    const val = std.mem.readInt(i32, buf[offset..][0..4], .little);
    return .{ .val = val, .next = offset + 4 };
}

/// Build a full wire message: header + encoded request/response
pub fn buildMessage(allocator: std.mem.Allocator, payload: []const u8) ![]u8 {
    const msg = try allocator.alloc(u8, HEADER_SIZE + payload.len);
    encodeHeader(msg[0..HEADER_SIZE], @intCast(payload.len));
    @memcpy(msg[HEADER_SIZE..], payload);
    return msg;
}

test "header encode/decode roundtrip" {
    var hdr: [HEADER_SIZE]u8 = undefined;
    encodeHeader(&hdr, 42);
    const len = try decodeHeader(&hdr);
    try std.testing.expectEqual(@as(u32, 42), len);
}

test "request encode/decode roundtrip" {
    const alloc = std.testing.allocator;
    const req = Request{
        .id = 1,
        .command = .ping,
        .payload = &[_]u8{},
    };
    const encoded = try encodeRequest(alloc, req);
    defer alloc.free(encoded);

    const decoded = try decodeRequest(encoded);
    try std.testing.expectEqual(@as(u32, 1), decoded.id);
    try std.testing.expectEqual(Command.ping, decoded.command);
    try std.testing.expectEqual(@as(usize, 0), decoded.payload.len);
}

test "response encode/decode roundtrip" {
    const alloc = std.testing.allocator;
    const resp = Response{
        .id = 5,
        .status = .ok,
        .payload = "hello",
    };
    const encoded = try encodeResponse(alloc, resp);
    defer alloc.free(encoded);

    const decoded = try decodeResponse(encoded);
    try std.testing.expectEqual(@as(u32, 5), decoded.id);
    try std.testing.expectEqual(Status.ok, decoded.status);
    try std.testing.expectEqualStrings("hello", decoded.payload);
}

test "payload helpers" {
    var buf: [256]u8 = undefined;
    var off: usize = 0;
    off = writeU32(&buf, off, 0xDEADBEEF);
    off = writeString(&buf, off, "test_string");
    off = writeU8(&buf, off, 0x42);

    var roff: usize = 0;
    const v1 = readU32(&buf, roff);
    roff = v1.next;
    try std.testing.expectEqual(@as(u32, 0xDEADBEEF), v1.val);

    const v2 = readString(&buf, roff);
    roff = v2.next;
    try std.testing.expectEqualStrings("test_string", v2.val);

    const v3 = readU8(&buf, roff);
    try std.testing.expectEqual(@as(u8, 0x42), v3.val);
}
