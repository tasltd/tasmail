// Added (TMAIL-127): Meeting slot suggester service.
//
// Given a list of busy intervals (typically merged from every attendee's
// calendar via the free-busy endpoint), a meeting duration, a working-hours
// window, a date range, and a maximum number of slots to return, find the
// first N contiguous gaps that are large enough to host the meeting.
//
// This module is intentionally pure — no DB, no async, no chrono timezone
// magic beyond UTC arithmetic. The handler is responsible for fetching busy
// intervals; the suggester only does interval math. That separation makes
// the algorithm trivially unit-testable and reusable for both server-side
// suggestions and future client-side what-if previews.

use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc, Weekday};

/// A half-open busy interval `[start, end)` on the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusyInterval {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Working hours constraint expressed in UTC minutes-of-day.
///
/// We deliberately work in UTC throughout the algorithm — the caller is
/// expected to translate the user's local working hours (e.g. 09:00–17:00
/// Africa/Accra which is UTC) into the equivalent UTC minute window. For
/// Ghana (the launch market) that translation is a no-op since GMT == UTC.
#[derive(Debug, Clone, Copy)]
pub struct WorkingHours {
    /// UTC minutes from midnight that the workday starts (inclusive).
    pub start_minute: u32,
    /// UTC minutes from midnight that the workday ends (exclusive).
    pub end_minute: u32,
    /// Whether weekends (Sat/Sun) are eligible. Defaults to false.
    pub include_weekends: bool,
}

impl Default for WorkingHours {
    fn default() -> Self {
        Self {
            start_minute: 9 * 60,
            end_minute: 17 * 60,
            include_weekends: false,
        }
    }
}

/// Inputs for [`suggest_slots`]. Grouped in a struct so the public API can grow
/// (e.g. preferred slot size, attendee weighting) without churning callers.
#[derive(Debug, Clone)]
pub struct SuggestSlotsInput {
    /// Combined busy intervals across all attendees. The function tolerates
    /// overlaps and unsorted input — they get merged internally.
    pub busy: Vec<BusyInterval>,
    /// Earliest moment a candidate slot may start (inclusive).
    pub range_start: DateTime<Utc>,
    /// Latest moment a candidate slot may end (exclusive).
    pub range_end: DateTime<Utc>,
    /// Meeting length. Must be > 0.
    pub duration: Duration,
    /// Working-hours window that bounds each candidate per UTC day.
    pub working_hours: WorkingHours,
    /// Maximum number of slots to return. Capped to 50 to avoid runaway
    /// response sizes if a caller passes something silly.
    pub max_slots: usize,
    /// Slot start alignment in minutes — typically 15 or 30 so candidates
    /// land on familiar UI times. Must be > 0.
    pub step_minutes: u32,
}

/// A candidate slot returned to the caller. `end - start == duration`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedSlot {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Hard cap on `max_slots` — chosen so the response stays under a few KB even
/// for week-long ranges. Callers that want more should paginate by advancing
/// `range_start`.
const MAX_SLOTS_HARD_CAP: usize = 50;

/// Hard cap on `duration` so a bug in caller code can't ask for a 10-day
/// meeting and force us to iterate every minute in the range.
const MAX_DURATION_HOURS: i64 = 24;

/// Find up to `max_slots` candidate slots inside `range_start..range_end` that
/// (a) sit fully inside the working-hours window for their day, and (b) do
/// not overlap any busy interval.
///
/// The algorithm is the classic interval-cover pattern:
///
/// 1. Clamp & validate inputs (reject zero/negative duration, swap reversed
///    range, etc.).
/// 2. Merge all busy intervals into a sorted, non-overlapping list.
/// 3. Walk forward in `step_minutes` increments. At each tick, check whether
///    `[tick, tick + duration)` fits inside the working-hours window for its
///    day and doesn't intersect any busy interval. If both checks pass, emit
///    the slot and jump forward by `duration` so the next candidate doesn't
///    immediately collide with the one we just emitted.
///
/// Complexity is `O(range_minutes / step_minutes * log(busy))` which is fine
/// for the realistic case of "next 14 days at 30-minute steps with <500 busy
/// intervals".
pub fn suggest_slots(input: SuggestSlotsInput) -> Result<Vec<SuggestedSlot>, String> {
    let SuggestSlotsInput {
        busy,
        range_start,
        range_end,
        duration,
        working_hours,
        max_slots,
        step_minutes,
    } = input;

    // ----- Validation ---------------------------------------------------
    if duration <= Duration::zero() {
        return Err("duration must be positive".to_string());
    }
    if duration > Duration::hours(MAX_DURATION_HOURS) {
        return Err(format!(
            "duration must be <= {MAX_DURATION_HOURS} hours"
        ));
    }
    if step_minutes == 0 {
        return Err("step_minutes must be > 0".to_string());
    }
    if range_end <= range_start {
        return Err("range_end must be after range_start".to_string());
    }
    if working_hours.start_minute >= working_hours.end_minute {
        return Err("working_hours.start_minute must be < end_minute".to_string());
    }
    if working_hours.end_minute > 24 * 60 {
        return Err("working_hours.end_minute must be <= 24*60".to_string());
    }

    let max_slots = max_slots.min(MAX_SLOTS_HARD_CAP);
    if max_slots == 0 {
        return Ok(Vec::new());
    }

    // ----- Merge busy intervals ----------------------------------------
    let merged = merge_busy(busy);

    // ----- Walk the timeline -------------------------------------------
    let step = Duration::minutes(step_minutes as i64);
    let mut cursor = align_to_step(range_start, step_minutes);
    if cursor < range_start {
        cursor += step;
    }

    let mut out: Vec<SuggestedSlot> = Vec::with_capacity(max_slots);
    while out.len() < max_slots {
        let slot_end = cursor + duration;
        if slot_end > range_end {
            break;
        }
        if fits_working_hours(cursor, slot_end, &working_hours)
            && !overlaps_any(cursor, slot_end, &merged)
        {
            out.push(SuggestedSlot {
                start: cursor,
                end: slot_end,
            });
            // Jump forward by the meeting length so the next candidate is
            // distinct rather than overlapping the one we just emitted.
            cursor = align_to_step(cursor + duration, step_minutes);
        } else {
            cursor += step;
        }
    }

    Ok(out)
}

/// Merge overlapping/contiguous busy intervals. Public so the free-busy
/// handler can use the same logic when assembling the response.
pub fn merge_busy(mut intervals: Vec<BusyInterval>) -> Vec<BusyInterval> {
    intervals.retain(|i| i.end > i.start);
    if intervals.is_empty() {
        return intervals;
    }
    intervals.sort_by_key(|i| i.start);

    let mut merged: Vec<BusyInterval> = Vec::with_capacity(intervals.len());
    for cur in intervals {
        if let Some(last) = merged.last_mut() {
            if cur.start <= last.end {
                if cur.end > last.end {
                    last.end = cur.end;
                }
                continue;
            }
        }
        merged.push(cur);
    }
    merged
}

/// Floor `t` to the nearest multiple of `step_minutes` minutes (rounding
/// toward the past). Used so candidate slot starts always look "round".
fn align_to_step(t: DateTime<Utc>, step_minutes: u32) -> DateTime<Utc> {
    let minute = t.minute();
    let aligned_minute = (minute / step_minutes) * step_minutes;
    Utc.with_ymd_and_hms(
        t.year(),
        t.month(),
        t.day(),
        t.hour(),
        aligned_minute,
        0,
    )
    .single()
    .unwrap_or(t)
}

/// Does `[start, end)` fall entirely inside the working-hours window for its
/// (UTC) day? A slot that crosses midnight is rejected by definition.
fn fits_working_hours(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    wh: &WorkingHours,
) -> bool {
    if !wh.include_weekends {
        match start.weekday() {
            Weekday::Sat | Weekday::Sun => return false,
        _ => {}
        }
    }
    // Reject slots that cross midnight UTC — they conceptually span two
    // working days and the simple minute-of-day math doesn't apply.
    if start.date_naive() != end.date_naive() {
        return false;
    }
    let start_min = start.hour() * 60 + start.minute();
    let end_min = end.hour() * 60 + end.minute();
    // end == 0 means the slot ends exactly at midnight, which is fine even
    // though it's not strictly inside the current day's minute window.
    let end_min = if end_min == 0 { 24 * 60 } else { end_min };
    start_min >= wh.start_minute && end_min <= wh.end_minute
}

/// Binary-search-friendly overlap check against the merged list.
fn overlaps_any(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    merged: &[BusyInterval],
) -> bool {
    // Linear scan is fine in practice — the merged list is small (<500 in
    // realistic free-busy windows) and we early-exit on the first hit.
    for b in merged {
        if b.start < end && b.end > start {
            return true;
        }
        if b.start >= end {
            // The list is sorted by start, so nothing further can overlap.
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).single().unwrap()
    }

    fn busy(s: DateTime<Utc>, e: DateTime<Utc>) -> BusyInterval {
        BusyInterval { start: s, end: e }
    }

    fn default_input(
        busy_list: Vec<BusyInterval>,
        range_start: DateTime<Utc>,
        range_end: DateTime<Utc>,
    ) -> SuggestSlotsInput {
        SuggestSlotsInput {
            busy: busy_list,
            range_start,
            range_end,
            duration: Duration::minutes(30),
            working_hours: WorkingHours::default(),
            max_slots: 5,
            step_minutes: 30,
        }
    }

    // ---- merge_busy ----------------------------------------------------

    #[test]
    fn merge_busy_handles_empty_input() {
        assert!(merge_busy(vec![]).is_empty());
    }

    #[test]
    fn merge_busy_drops_zero_length_intervals() {
        let t = utc(2026, 6, 1, 10, 0);
        let merged = merge_busy(vec![busy(t, t)]);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_busy_merges_overlap() {
        let merged = merge_busy(vec![
            busy(utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 10, 0)),
            busy(utc(2026, 6, 1, 9, 30), utc(2026, 6, 1, 11, 0)),
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start, utc(2026, 6, 1, 9, 0));
        assert_eq!(merged[0].end, utc(2026, 6, 1, 11, 0));
    }

    #[test]
    fn merge_busy_merges_touching_intervals() {
        let merged = merge_busy(vec![
            busy(utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 10, 0)),
            busy(utc(2026, 6, 1, 10, 0), utc(2026, 6, 1, 11, 0)),
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].end, utc(2026, 6, 1, 11, 0));
    }

    #[test]
    fn merge_busy_keeps_disjoint_intervals() {
        let merged = merge_busy(vec![
            busy(utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 10, 0)),
            busy(utc(2026, 6, 1, 14, 0), utc(2026, 6, 1, 15, 0)),
        ]);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_busy_sorts_unordered_input() {
        let merged = merge_busy(vec![
            busy(utc(2026, 6, 1, 14, 0), utc(2026, 6, 1, 15, 0)),
            busy(utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 10, 0)),
        ]);
        assert_eq!(merged[0].start, utc(2026, 6, 1, 9, 0));
        assert_eq!(merged[1].start, utc(2026, 6, 1, 14, 0));
    }

    // ---- input validation ----------------------------------------------

    #[test]
    fn suggest_slots_rejects_zero_duration() {
        let mut input = default_input(vec![], utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        input.duration = Duration::zero();
        assert!(suggest_slots(input).is_err());
    }

    #[test]
    fn suggest_slots_rejects_negative_duration() {
        let mut input = default_input(vec![], utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        input.duration = Duration::minutes(-30);
        assert!(suggest_slots(input).is_err());
    }

    #[test]
    fn suggest_slots_rejects_overly_long_duration() {
        let mut input = default_input(vec![], utc(2026, 6, 1, 9, 0), utc(2026, 6, 5, 17, 0));
        input.duration = Duration::hours(25);
        assert!(suggest_slots(input).is_err());
    }

    #[test]
    fn suggest_slots_rejects_zero_step() {
        let mut input = default_input(vec![], utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        input.step_minutes = 0;
        assert!(suggest_slots(input).is_err());
    }

    #[test]
    fn suggest_slots_rejects_reversed_range() {
        let input = default_input(vec![], utc(2026, 6, 5, 9, 0), utc(2026, 6, 1, 17, 0));
        assert!(suggest_slots(input).is_err());
    }

    #[test]
    fn suggest_slots_rejects_invalid_working_hours() {
        let mut input = default_input(vec![], utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        input.working_hours = WorkingHours {
            start_minute: 1000,
            end_minute: 900,
            include_weekends: false,
        };
        assert!(suggest_slots(input).is_err());
    }

    // ---- core algorithm -------------------------------------------------

    #[test]
    fn finds_first_slot_when_calendar_is_empty() {
        // Monday 2026-06-01 — fully free.
        let input = default_input(vec![], utc(2026, 6, 1, 8, 0), utc(2026, 6, 1, 18, 0));
        let slots = suggest_slots(input).unwrap();
        assert!(!slots.is_empty());
        assert_eq!(slots[0].start, utc(2026, 6, 1, 9, 0));
        assert_eq!(slots[0].end, utc(2026, 6, 1, 9, 30));
    }

    #[test]
    fn returns_requested_count_when_room_exists() {
        // 5 free 30-minute slots in an empty 8-hour window means we should
        // get exactly max_slots back.
        let mut input = default_input(vec![], utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        input.max_slots = 5;
        let slots = suggest_slots(input).unwrap();
        assert_eq!(slots.len(), 5);
        // Slots should be ordered and non-overlapping.
        for w in slots.windows(2) {
            assert!(w[0].end <= w[1].start);
        }
    }

    #[test]
    fn skips_over_busy_intervals() {
        // Block 9-11. First available slot at 11:00.
        let busy_list = vec![busy(utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 11, 0))];
        let input = default_input(busy_list, utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        let slots = suggest_slots(input).unwrap();
        assert_eq!(slots[0].start, utc(2026, 6, 1, 11, 0));
    }

    #[test]
    fn respects_working_hours_window() {
        // 8 AM should NOT be returned — working hours default to 09:00.
        let mut input = default_input(vec![], utc(2026, 6, 1, 7, 0), utc(2026, 6, 1, 18, 0));
        input.max_slots = 1;
        let slots = suggest_slots(input).unwrap();
        assert_eq!(slots[0].start.hour(), 9);
    }

    #[test]
    fn last_slot_must_end_within_working_hours() {
        // The 16:45–17:15 slot crosses the 17:00 working-hours boundary and
        // must be rejected. The previous 16:30–17:00 slot is fine.
        let mut input = default_input(vec![], utc(2026, 6, 1, 16, 30), utc(2026, 6, 1, 18, 0));
        input.max_slots = 2;
        let slots = suggest_slots(input).unwrap();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].start, utc(2026, 6, 1, 16, 30));
        assert_eq!(slots[0].end, utc(2026, 6, 1, 17, 0));
    }

    #[test]
    fn skips_weekend_by_default() {
        // Saturday 2026-06-06. No slots should be returned even though the
        // window is otherwise valid.
        let input = default_input(vec![], utc(2026, 6, 6, 9, 0), utc(2026, 6, 6, 17, 0));
        let slots = suggest_slots(input).unwrap();
        assert!(slots.is_empty());
    }

    #[test]
    fn includes_weekend_when_opted_in() {
        let mut input = default_input(vec![], utc(2026, 6, 6, 9, 0), utc(2026, 6, 6, 17, 0));
        input.working_hours.include_weekends = true;
        let slots = suggest_slots(input).unwrap();
        assert!(!slots.is_empty());
    }

    #[test]
    fn merges_overlapping_busy_before_search() {
        // Two overlapping busy intervals that together block the whole
        // workday. We expect zero slots.
        let busy_list = vec![
            busy(utc(2026, 6, 1, 8, 0), utc(2026, 6, 1, 13, 0)),
            busy(utc(2026, 6, 1, 12, 30), utc(2026, 6, 1, 18, 0)),
        ];
        let input = default_input(busy_list, utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        let slots = suggest_slots(input).unwrap();
        assert!(slots.is_empty());
    }

    #[test]
    fn max_slots_zero_returns_empty() {
        let mut input = default_input(vec![], utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        input.max_slots = 0;
        let slots = suggest_slots(input).unwrap();
        assert!(slots.is_empty());
    }

    #[test]
    fn caps_max_slots_at_hard_limit() {
        let mut input = default_input(vec![], utc(2026, 6, 1, 9, 0), utc(2026, 6, 5, 17, 0));
        input.max_slots = 9999;
        let slots = suggest_slots(input).unwrap();
        assert!(slots.len() <= MAX_SLOTS_HARD_CAP);
    }

    #[test]
    fn step_alignment_rounds_starts() {
        // Range starts at 09:07 with a 30-minute step — first slot should
        // start at 09:30, not 09:07.
        let mut input = default_input(vec![], utc(2026, 6, 1, 9, 7), utc(2026, 6, 1, 17, 0));
        input.max_slots = 1;
        let slots = suggest_slots(input).unwrap();
        assert_eq!(slots[0].start, utc(2026, 6, 1, 9, 30));
    }

    #[test]
    fn handles_back_to_back_meetings() {
        // 9–10, 10–11, 11–12 booked solid. First gap is 12:00.
        let busy_list = vec![
            busy(utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 10, 0)),
            busy(utc(2026, 6, 1, 10, 0), utc(2026, 6, 1, 11, 0)),
            busy(utc(2026, 6, 1, 11, 0), utc(2026, 6, 1, 12, 0)),
        ];
        let mut input = default_input(busy_list, utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        input.max_slots = 1;
        let slots = suggest_slots(input).unwrap();
        assert_eq!(slots[0].start, utc(2026, 6, 1, 12, 0));
    }

    #[test]
    fn longer_duration_skips_small_gaps() {
        // Two free hours total, but the longest single gap is 60 min.
        // A 90-minute meeting should not fit.
        let busy_list = vec![
            busy(utc(2026, 6, 1, 10, 0), utc(2026, 6, 1, 11, 0)),
            busy(utc(2026, 6, 1, 12, 0), utc(2026, 6, 1, 13, 0)),
            busy(utc(2026, 6, 1, 14, 0), utc(2026, 6, 1, 15, 0)),
            busy(utc(2026, 6, 1, 16, 0), utc(2026, 6, 1, 17, 0)),
        ];
        let mut input = default_input(busy_list, utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        input.duration = Duration::minutes(90);
        input.max_slots = 5;
        let slots = suggest_slots(input).unwrap();
        assert!(slots.is_empty());
    }

    #[test]
    fn returned_slots_have_correct_duration() {
        let mut input = default_input(vec![], utc(2026, 6, 1, 9, 0), utc(2026, 6, 1, 17, 0));
        input.duration = Duration::minutes(45);
        input.step_minutes = 15;
        input.max_slots = 3;
        let slots = suggest_slots(input).unwrap();
        for slot in slots {
            assert_eq!(slot.end - slot.start, Duration::minutes(45));
        }
    }
}
