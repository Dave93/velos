const std = @import("std");

/// A parsed cron expression with bitfields for each time component.
/// Supports standard 5-field format: minute hour day month weekday
pub const CronExpr = struct {
    minutes: [60]bool,
    hours: [24]bool,
    days: [31]bool, // index 0 = day 1, etc.
    months: [12]bool, // index 0 = month 1, etc.
    weekdays: [7]bool, // 0 = Sunday, 6 = Saturday

    pub const ParseError = error{
        InvalidFormat,
        InvalidField,
        InvalidRange,
        InvalidStep,
        ValueOutOfRange,
    };

    /// Parse a 5-field cron expression string.
    /// Format: "minute hour day month weekday"
    /// Supports: * (all), N (specific), N-M (range), */N (step), N,M,O (list)
    pub fn parse(expr: []const u8) ParseError!CronExpr {
        var result = CronExpr{
            .minutes = [_]bool{false} ** 60,
            .hours = [_]bool{false} ** 24,
            .days = [_]bool{false} ** 31,
            .months = [_]bool{false} ** 12,
            .weekdays = [_]bool{false} ** 7,
        };

        // Split by whitespace into fields
        var fields: [5][]const u8 = undefined;
        var field_count: usize = 0;

        var iter = std.mem.tokenizeAny(u8, expr, " \t");
        while (iter.next()) |token| {
            if (field_count >= 5) return ParseError.InvalidFormat;
            fields[field_count] = token;
            field_count += 1;
        }

        if (field_count != 5) return ParseError.InvalidFormat;

        try parseField(fields[0], &result.minutes, 0, 59);
        try parseField(fields[1], &result.hours, 0, 23);
        try parseField(fields[2], &result.days, 1, 31);
        try parseField(fields[3], &result.months, 1, 12);
        try parseField(fields[4], &result.weekdays, 0, 6);

        return result;
    }

    /// Check if the given time components match this cron expression.
    /// `day` is 1-31, `month` is 1-12, `weekday` is 0-6 (Sunday=0).
    pub fn matches(self: *const CronExpr, minute: u8, hour: u8, day: u8, month: u8, weekday: u8) bool {
        if (minute >= 60) return false;
        if (hour >= 24) return false;
        if (day == 0 or day > 31) return false;
        if (month == 0 or month > 12) return false;
        if (weekday > 6) return false;

        return self.minutes[minute] and
            self.hours[hour] and
            self.days[day - 1] and
            self.months[month - 1] and
            self.weekdays[weekday];
    }

    // ---- Field parsing ----

    /// Parse a single cron field (e.g., "*/5", "1-15", "1,3,5", "*", "30").
    /// `field_arr` is a bool array. `min` and `max` are the valid range.
    /// For days, min=1, max=31 (stored at index 0..30).
    /// For months, min=1, max=12 (stored at index 0..11).
    fn parseField(field: []const u8, field_arr: []bool, min: u8, max: u8) ParseError!void {
        // Handle comma-separated list (e.g., "1,3,5")
        var list_iter = std.mem.splitScalar(u8, field, ',');
        while (list_iter.next()) |part| {
            if (part.len == 0) return ParseError.InvalidField;
            try parsePart(part, field_arr, min, max);
        }
    }

    /// Parse a single part of a cron field (no commas — handles *, N, N-M, */N, N-M/S).
    fn parsePart(part: []const u8, field_arr: []bool, min: u8, max: u8) ParseError!void {
        // Check for step: anything/N
        if (std.mem.indexOfScalar(u8, part, '/')) |slash_pos| {
            const range_part = part[0..slash_pos];
            const step_str = part[slash_pos + 1 ..];
            const step = parseNum(step_str) orelse return ParseError.InvalidStep;
            if (step == 0) return ParseError.InvalidStep;

            var range_min = min;
            var range_max = max;

            if (range_part.len == 1 and range_part[0] == '*') {
                // */N — step over full range
            } else {
                // N-M/S — step over a range
                const range = parseRange(range_part, min, max) orelse return ParseError.InvalidRange;
                range_min = range.start;
                range_max = range.end;
            }

            var v: u8 = range_min;
            while (v <= range_max) : (v += step) {
                const idx = v - min;
                if (idx < field_arr.len) field_arr[idx] = true;
                // Prevent overflow
                if (@as(u16, v) + step > 255) break;
            }
            return;
        }

        // Wildcard: *
        if (part.len == 1 and part[0] == '*') {
            var v: u8 = min;
            while (v <= max) : (v += 1) {
                field_arr[v - min] = true;
                if (v == max) break; // prevent u8 overflow
            }
            return;
        }

        // Range: N-M
        if (std.mem.indexOfScalar(u8, part, '-')) |_| {
            const range = parseRange(part, min, max) orelse return ParseError.InvalidRange;
            var v: u8 = range.start;
            while (v <= range.end) : (v += 1) {
                field_arr[v - min] = true;
                if (v == range.end) break; // prevent u8 overflow
            }
            return;
        }

        // Single number
        const num = parseNum(part) orelse return ParseError.InvalidField;
        if (num < min or num > max) return ParseError.ValueOutOfRange;
        field_arr[num - min] = true;
    }

    const Range = struct { start: u8, end: u8 };

    fn parseRange(part: []const u8, min: u8, max: u8) ?Range {
        const dash_pos = std.mem.indexOfScalar(u8, part, '-') orelse return null;
        const start = parseNum(part[0..dash_pos]) orelse return null;
        const end = parseNum(part[dash_pos + 1 ..]) orelse return null;
        if (start < min or end > max or start > end) return null;
        return Range{ .start = start, .end = end };
    }

    fn parseNum(s: []const u8) ?u8 {
        if (s.len == 0) return null;
        var result: u16 = 0;
        for (s) |c| {
            if (c < '0' or c > '9') return null;
            result = result * 10 + (c - '0');
            if (result > 255) return null;
        }
        return @intCast(result);
    }
};

// ============================================================
// Tests
// ============================================================

test "parse simple cron - specific minute and hour" {
    const expr = try CronExpr.parse("30 2 * * *");
    // 30th minute, 2nd hour, any day/month/weekday
    try std.testing.expect(expr.matches(30, 2, 15, 6, 3));
    try std.testing.expect(!expr.matches(31, 2, 15, 6, 3));
    try std.testing.expect(!expr.matches(30, 3, 15, 6, 3));
}

test "parse wildcard - every minute" {
    const expr = try CronExpr.parse("* * * * *");
    try std.testing.expect(expr.matches(0, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(59, 23, 31, 12, 6));
    try std.testing.expect(expr.matches(30, 12, 15, 6, 3));
}

test "parse step - every 5 minutes" {
    const expr = try CronExpr.parse("*/5 * * * *");
    try std.testing.expect(expr.matches(0, 12, 1, 1, 0));
    try std.testing.expect(expr.matches(5, 12, 1, 1, 0));
    try std.testing.expect(expr.matches(10, 12, 1, 1, 0));
    try std.testing.expect(expr.matches(55, 12, 1, 1, 0));
    try std.testing.expect(!expr.matches(3, 12, 1, 1, 0));
    try std.testing.expect(!expr.matches(7, 12, 1, 1, 0));
}

test "parse step - every 2 hours" {
    const expr = try CronExpr.parse("0 */2 * * *");
    try std.testing.expect(expr.matches(0, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(0, 2, 1, 1, 0));
    try std.testing.expect(expr.matches(0, 22, 1, 1, 0));
    try std.testing.expect(!expr.matches(0, 1, 1, 1, 0));
    try std.testing.expect(!expr.matches(0, 3, 1, 1, 0));
}

test "parse range - minutes 10 to 20" {
    const expr = try CronExpr.parse("10-20 * * * *");
    try std.testing.expect(expr.matches(10, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(15, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(20, 0, 1, 1, 0));
    try std.testing.expect(!expr.matches(9, 0, 1, 1, 0));
    try std.testing.expect(!expr.matches(21, 0, 1, 1, 0));
}

test "parse list - specific minutes" {
    const expr = try CronExpr.parse("0,15,30,45 * * * *");
    try std.testing.expect(expr.matches(0, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(15, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(30, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(45, 0, 1, 1, 0));
    try std.testing.expect(!expr.matches(1, 0, 1, 1, 0));
    try std.testing.expect(!expr.matches(14, 0, 1, 1, 0));
}

test "parse combined - workdays at 9:30" {
    // Monday to Friday (1-5) at 9:30
    const expr = try CronExpr.parse("30 9 * * 1-5");
    try std.testing.expect(expr.matches(30, 9, 15, 6, 1)); // Monday
    try std.testing.expect(expr.matches(30, 9, 15, 6, 5)); // Friday
    try std.testing.expect(!expr.matches(30, 9, 15, 6, 0)); // Sunday
    try std.testing.expect(!expr.matches(30, 9, 15, 6, 6)); // Saturday
    try std.testing.expect(!expr.matches(31, 9, 15, 6, 1)); // wrong minute
}

test "parse range with step" {
    const expr = try CronExpr.parse("1-30/5 * * * *");
    try std.testing.expect(expr.matches(1, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(6, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(11, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(16, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(21, 0, 1, 1, 0));
    try std.testing.expect(expr.matches(26, 0, 1, 1, 0));
    try std.testing.expect(!expr.matches(0, 0, 1, 1, 0));
    try std.testing.expect(!expr.matches(2, 0, 1, 1, 0));
    try std.testing.expect(!expr.matches(31, 0, 1, 1, 0));
}

test "parse specific day and month" {
    // 25th of December at midnight
    const expr = try CronExpr.parse("0 0 25 12 *");
    try std.testing.expect(expr.matches(0, 0, 25, 12, 3));
    try std.testing.expect(!expr.matches(0, 0, 24, 12, 3));
    try std.testing.expect(!expr.matches(0, 0, 25, 11, 3));
}

test "parse error - too few fields" {
    const result = CronExpr.parse("30 2 *");
    try std.testing.expectError(CronExpr.ParseError.InvalidFormat, result);
}

test "parse error - too many fields" {
    const result = CronExpr.parse("30 2 * * * *");
    try std.testing.expectError(CronExpr.ParseError.InvalidFormat, result);
}

test "parse error - value out of range" {
    const result = CronExpr.parse("60 * * * *");
    try std.testing.expectError(CronExpr.ParseError.ValueOutOfRange, result);
}

test "parse error - invalid step" {
    const result = CronExpr.parse("*/0 * * * *");
    try std.testing.expectError(CronExpr.ParseError.InvalidStep, result);
}

test "matches rejects invalid inputs" {
    const expr = try CronExpr.parse("* * * * *");
    try std.testing.expect(!expr.matches(60, 0, 1, 1, 0)); // minute out of range
    try std.testing.expect(!expr.matches(0, 24, 1, 1, 0)); // hour out of range
    try std.testing.expect(!expr.matches(0, 0, 0, 1, 0)); // day 0
    try std.testing.expect(!expr.matches(0, 0, 32, 1, 0)); // day 32
    try std.testing.expect(!expr.matches(0, 0, 1, 0, 0)); // month 0
    try std.testing.expect(!expr.matches(0, 0, 1, 13, 0)); // month 13
    try std.testing.expect(!expr.matches(0, 0, 1, 1, 7)); // weekday 7
}
