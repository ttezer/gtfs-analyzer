//! Zip-bomb / unbounded-decompression protection.
//!
//! `GuardedReader` counts bytes **actually decompressed** from the wrapped `Read` source
//! and stops reading when an archive entry or the entire archive traversal exceeds its limits.
//! The check runs during inflate (not post-hoc): `read` returns an error as soon as a limit is
//! exceeded, cutting the stream before an OOM can occur.
//!
//! Design notes:
//! - The ratio denominator is always `zf.compressed_size()` (the actual bytes available to us);
//!   the declared decompressed size (`zf.size()`) is attacker-controlled and never used.
//! - `ratio_floor`: to avoid false positives on small but highly compressed legitimate inputs
//!   (e.g. repetitive integer CSV), apply the ratio check only above this size.
//! - The constants in `DEFAULT_DECOMPRESSION_LIMITS` are placeholders; calibrate them against
//!   a real ~9.4 GB feed measurement and the wasm64 memory envelope.

use std::io::{self, Read};

/// Decompression guard limits (bytes; ratio is dimensionless).
#[derive(Clone, Copy, Debug)]
pub struct DecompressionLimits {
    /// Absolute cap on total decompressed bytes in one archive traversal.
    pub max_total_decompressed: u64,
    /// Absolute cap on decompressed bytes for one entry.
    pub max_entry_decompressed: u64,
    /// Do not apply the ratio check below this decompressed size (small inputs are harmless).
    pub ratio_floor: u64,
    /// Maximum allowed decompressed:compressed ratio above the floor.
    pub max_ratio: u64,
}

/// Default decompression guard limits — calibrated in 2026-07 by scanning the central directory
/// of a real 419 MB DELFI aggregate feed (46.3M stop_times) (issue #46; report in
/// `log_zip_ratio_report` inside `list_zip_files`).
///
/// Measured (per-entry, NO decompression; the guard did not false-trip):
/// - maximum ratio 17.8:1 (an `.out` artifact); maximum GTFS file trips.txt 12.3:1,
/// - largest entry stop_times ~2.74 GiB; total decompressed ~3.39 GiB; feed ratio ~8.3:1.
///
/// **2026-08-01 — Entur (Norway) became the NEW WORST RATIO; the calibration baseline was updated:**
/// - `calendar_dates.txt` **32.8:1** (31.2 MiB decompressed / 1.0 MiB) — ~1.8× DELFI's 17.8:1
///   and ABOVE the `ratio_floor` (16 MiB), so it is SUBJECT to the ratio check,
/// - largest entry `shapes.txt` 2,651.8 MiB; total decompressed 3,640.2 MiB (feed ratio 6.2:1).
///
/// Entur did not exceed the limit of 80, but the margin fell from **4.5×** to **2.4×**.
/// `calendar_dates` is highly compressible by nature (repeating date + service_id), and a
/// legitimate feed 2.4× more repetitive than Entur would be REJECTED with FATAL ARC_029 —
/// the most severe possible false positive. Limit 80 → 100.
///
/// Values are well above the measured worst case (so legitimate large feeds are not rejected),
/// but narrow enough to catch real zip-bomb patterns (tiny ZIP → enormous expansion).
/// This is the ONE place to retune: rerun the report on a new worst-case feed and update it
/// while preserving a margin.
pub const DEFAULT_DECOMPRESSION_LIMITS: DecompressionLimits = DecompressionLimits {
    // 16 GiB — deliberately far above the measured ~3.4 GiB total: larger real feeds
    // (~9.4 GB class) exist, and a "2× total" limit (~7 GiB) would reject them.
    // Just below the wasm64 OOM wall (~16 GiB). NOTE: this is NOT the real OOM barrier for
    // a normal feed (peak memory is ~14 GiB from K6 analytics); this cap is only for
    // decompression-dominant bombs.
    max_total_decompressed: 16 * 1024 * 1024 * 1024, // 16 GiB
    // ~4.4× the measured ~2.74 GiB largest entry. A larger single entry cannot be processed
    // past the 16 GiB wall → no false-trip risk; ratio is the main check for large entries.
    max_entry_decompressed: 12 * 1024 * 1024 * 1024, // 12 GiB
    // Ignore the ratio below this decompressed size (small, highly compressed inputs are harmless).
    ratio_floor: 16 * 1024 * 1024, // 16 MiB
    // Measured worst per-entry ratio **32.8:1** (Entur `calendar_dates.txt`, 2026-08-01)
    // → 100 leaves ~3× margin. A bomb needs ~200:1+ and a single DEFLATE stream can reach
    // ~1032:1, so 100 is still one order below the bomb region; entry/total caps close the
    // gap (absolute damage is limited to 12/16 GiB regardless of ratio).
    max_ratio: 100,
};

/// Decompression limits for the SDK running with smaller wasm32 memory.
///
/// The placeholder limits used by the native CLI for large-feed support are too loose for
/// WASM. The SDK applies its own input-size gate; this is the second line of defense and is
/// particularly necessary on paths where K2 reopens the ZIP.
pub const SDK_DECOMPRESSION_LIMITS: DecompressionLimits = DecompressionLimits {
    max_total_decompressed: 6 * 1024 * 1024 * 1024,
    max_entry_decompressed: 3 * 1024 * 1024 * 1024,
    ratio_floor: 16 * 1024 * 1024,
    max_ratio: 100,
};

/// SDK upper bound for one file loaded into a raw Vec in K1 (not streamed).
pub const SDK_MAX_BUFFERED_FILE_BYTES: u64 = 512 * 1024 * 1024;

/// Total budget applied to a K2 ZIP stream's row reader.
pub struct DecompressionBudget {
    limits: DecompressionLimits,
    total_decompressed: u64,
}

impl DecompressionBudget {
    pub fn new(limits: DecompressionLimits) -> Self {
        Self {
            limits,
            total_decompressed: 0,
        }
    }

    pub fn reader<'a, R: Read>(
        &'a mut self,
        inner: R,
        compressed_size: u64,
    ) -> GuardedReader<'a, R> {
        GuardedReader::new(
            inner,
            self.limits,
            &mut self.total_decompressed,
            compressed_size,
        )
    }
}

/// Reason for a limit breach (for error messages and diagnosis).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardTrip {
    /// Absolute decompressed-byte cap per entry was exceeded.
    EntryCap { got: u64, cap: u64 },
    /// Archive-wide total decompressed-byte cap was exceeded.
    TotalCap { got: u64, cap: u64 },
    /// Above-floor decompressed:compressed ratio cap was exceeded.
    Ratio {
        decompressed: u64,
        compressed: u64,
        cap: u64,
    },
}

impl GuardTrip {
    /// Fatal message-template variant and parameters (for CLI/SDK/UI translation).
    pub fn fatal_params(&self) -> (&'static str, Vec<(&'static str, u64)>) {
        match *self {
            GuardTrip::EntryCap { got, cap } => ("entry_cap", vec![("got", got), ("cap", cap)]),
            GuardTrip::TotalCap { got, cap } => ("total_cap", vec![("got", got), ("cap", cap)]),
            GuardTrip::Ratio {
                decompressed,
                compressed,
                cap,
            } => (
                "ratio",
                vec![
                    ("decompressed", decompressed),
                    ("compressed", compressed),
                    ("cap", cap),
                ],
            ),
        }
    }

    /// Human-readable description (embedded in the FatalError message).
    pub fn describe(&self) -> String {
        match self {
            GuardTrip::EntryCap { got, cap } =>
                format!("girdi başına açılmış-boyut sınırı aşıldı ({got} > {cap} bayt)"),
            GuardTrip::TotalCap { got, cap } =>
                format!("arşiv geneli açılmış-boyut sınırı aşıldı ({got} > {cap} bayt)"),
            GuardTrip::Ratio { decompressed, compressed, cap } =>
                format!("sıkıştırma oranı {cap}:1 sınırını aştı (açılmış {decompressed}, sıkıştırılmış {compressed} bayt)"),
        }
    }
}

/// `Read` adapter that counts decompressed bytes. When a limit is exceeded, `read` returns an
/// error whose reason is available through [`GuardedReader::tripped`]; the caller can distinguish
/// it from a plain ZIP error and select the appropriate `FatalCode`.
///
/// `total` is shared across all entries in a traversal (held by the outer loop).
pub struct GuardedReader<'a, R: Read> {
    inner: R,
    limits: DecompressionLimits,
    total: &'a mut u64,
    entry_compressed: u64,
    entry_decompressed: u64,
    tripped: Option<GuardTrip>,
}

impl<'a, R: Read> GuardedReader<'a, R> {
    pub fn new(
        inner: R,
        limits: DecompressionLimits,
        total: &'a mut u64,
        entry_compressed: u64,
    ) -> Self {
        Self {
            inner,
            limits,
            total,
            entry_compressed,
            entry_decompressed: 0,
            tripped: None,
        }
    }

    /// Returns the reason if a limit was exceeded (otherwise `None`).
    pub fn tripped(&self) -> Option<GuardTrip> {
        self.tripped
    }
}

fn tripped_err() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "decompression guard tripped")
}

impl<R: Read> Read for GuardedReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.tripped.is_some() {
            return Err(tripped_err()); // already tripped → no more reading
        }
        let n = self.inner.read(buf)?;
        if n == 0 {
            return Ok(0);
        }
        let n64 = n as u64;
        self.entry_decompressed += n64;
        *self.total += n64;

        // Multiplication-based ratio check: no division by zero or overflow. An above-floor
        // zero-compressed input has an infinite ratio → it trips correctly.
        let breach = if self.entry_decompressed > self.limits.max_entry_decompressed {
            Some(GuardTrip::EntryCap {
                got: self.entry_decompressed,
                cap: self.limits.max_entry_decompressed,
            })
        } else if *self.total > self.limits.max_total_decompressed {
            Some(GuardTrip::TotalCap {
                got: *self.total,
                cap: self.limits.max_total_decompressed,
            })
        } else if self.entry_decompressed > self.limits.ratio_floor
            && self.entry_decompressed > self.limits.max_ratio.saturating_mul(self.entry_compressed)
        {
            Some(GuardTrip::Ratio {
                decompressed: self.entry_decompressed,
                compressed: self.entry_compressed,
                cap: self.limits.max_ratio,
            })
        } else {
            None
        };

        if let Some(t) = breach {
            self.tripped = Some(t);
            return Err(tripped_err());
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LIMITS: DecompressionLimits = DecompressionLimits {
        max_total_decompressed: 1000,
        max_entry_decompressed: 400,
        ratio_floor: 100,
        max_ratio: 10,
    };

    fn read_all<R: Read>(r: &mut R) -> io::Result<usize> {
        let mut buf = Vec::new();
        r.read_to_end(&mut buf).map(|_| buf.len())
    }

    #[test]
    fn passes_within_limits() {
        let data = vec![0u8; 300];
        let mut total = 0u64;
        // compressed=300 → ratio 1:1, entry 300<400, total 300<1000 → passes
        let mut g = GuardedReader::new(&data[..], TEST_LIMITS, &mut total, 300);
        assert_eq!(read_all(&mut g).unwrap(), 300);
        assert_eq!(g.tripped(), None);
        assert_eq!(total, 300);
    }

    #[test]
    fn trips_on_entry_cap() {
        let data = vec![0u8; 500];
        let mut total = 0u64;
        // entry 500>400 → EntryCap (compressed is large, ratio does not trip)
        let mut g = GuardedReader::new(&data[..], TEST_LIMITS, &mut total, 500);
        assert!(read_all(&mut g).is_err());
        assert!(matches!(g.tripped(), Some(GuardTrip::EntryCap { .. })));
    }

    #[test]
    fn trips_on_ratio_above_floor() {
        let data = [0u8; 200];
        let mut total = 0u64;
        // entry 200>ratio_floor 100 and 200 > 10*compressed(1)=10 → Ratio
        let mut g = GuardedReader::new(&data[..], TEST_LIMITS, &mut total, 1);
        assert!(read_all(&mut g).is_err());
        assert!(matches!(g.tripped(), Some(GuardTrip::Ratio { .. })));
    }

    #[test]
    fn small_high_ratio_entry_below_floor_passes() {
        let data = [0u8; 50];
        let mut total = 0u64;
        // 50<=ratio_floor 100 → ratio ignored; below entry/total limits → passes
        let mut g = GuardedReader::new(&data[..], TEST_LIMITS, &mut total, 1);
        assert_eq!(read_all(&mut g).unwrap(), 50);
        assert_eq!(g.tripped(), None);
    }

    #[test]
    fn trips_on_total_across_entries() {
        let mut total = 0u64;
        // Two entries: each is under the limit, but their total exceeds total_cap.
        let d1 = vec![0u8; 350];
        {
            let mut g = GuardedReader::new(&d1[..], TEST_LIMITS, &mut total, 350);
            assert_eq!(read_all(&mut g).unwrap(), 350);
        }
        let d2 = vec![0u8; 350];
        {
            let mut g = GuardedReader::new(&d2[..], TEST_LIMITS, &mut total, 350);
            assert_eq!(read_all(&mut g).unwrap(), 350);
        }
        // total=700 < 1000; the third entry trips the cap
        let d3 = vec![0u8; 350];
        let mut g = GuardedReader::new(&d3[..], TEST_LIMITS, &mut total, 350);
        assert!(read_all(&mut g).is_err());
        assert!(matches!(g.tripped(), Some(GuardTrip::TotalCap { .. })));
    }
}
