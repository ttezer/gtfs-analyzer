//! The feed ZIP as one byte slice, without copying the file into the heap where possible.
//!
//! The pipeline reads the archive as `&[u8]` (the ZIP central directory sits at the end, and
//! stop_times/shapes are streamed straight from it), so the whole file must be addressable. On
//! Unix the file is memory-mapped read-only: its pages are file-backed, so under memory
//! pressure the kernel can drop them and read them again, whereas a `Vec<u8>` copy is anonymous
//! memory that stays for the whole run (1.9 GB on mdb-2014). Stdin, empty files, other
//! platforms and any mmap failure fall back to reading into memory as before.
//!
//! A mapped file must not be truncated while the analysis runs (the process would receive
//! SIGBUS); replacing it with a new file (write + rename) is safe.

use std::ops::Deref;
use std::path::Path;

pub enum FeedBytes {
    Owned(Vec<u8>),
    #[cfg(unix)]
    Mapped(Mapped),
}

impl Deref for FeedBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            FeedBytes::Owned(bytes) => bytes,
            #[cfg(unix)]
            FeedBytes::Mapped(map) => map.as_bytes(),
        }
    }
}

/// Reads the feed ZIP, or the whole of stdin when the path is `-`.
pub fn read_feed(path: &Path) -> Result<FeedBytes, String> {
    if path.as_os_str() == "-" {
        let mut buf = Vec::new();
        return std::io::Read::read_to_end(&mut std::io::stdin().lock(), &mut buf)
            .map(|_| FeedBytes::Owned(buf))
            .map_err(|err| format!("failed to read the feed from stdin: {err}"));
    }

    #[cfg(unix)]
    if std::env::var_os("GTFS_NO_MMAP").is_none() {
        if let Ok(file) = std::fs::File::open(path) {
            if let Some(map) = Mapped::new(&file) {
                return Ok(FeedBytes::Mapped(map));
            }
        }
    }

    std::fs::read(path)
        .map(FeedBytes::Owned)
        .map_err(|err| format!("failed to read '{}': {err}", path.display()))
}

#[cfg(unix)]
pub struct Mapped {
    ptr: *mut libc::c_void,
    len: usize,
}

#[cfg(unix)]
impl Mapped {
    /// `None` for an empty or non-regular file, or when mmap fails; the caller then reads the
    /// file normally, so the error (if any) is reported the usual way.
    fn new(file: &std::fs::File) -> Option<Self> {
        use std::os::fd::AsRawFd;
        let meta = file.metadata().ok()?;
        if !meta.is_file() {
            return None;
        }
        let len = usize::try_from(meta.len()).ok()?;
        if len == 0 {
            return None;
        }
        // SAFETY: a read-only private mapping of a regular file's first `len` bytes; the fd
        // can be closed after mmap returns, the mapping stays valid until munmap in Drop.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                file.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return None;
        }
        Some(Self { ptr, len })
    }

    fn as_bytes(&self) -> &[u8] {
        // SAFETY: `ptr` maps `len` readable bytes for the lifetime of `self` (unmapped in Drop),
        // and the mapping is never written through.
        unsafe { std::slice::from_raw_parts(self.ptr.cast::<u8>(), self.len) }
    }
}

#[cfg(unix)]
impl Drop for Mapped {
    fn drop(&mut self) {
        // SAFETY: `ptr`/`len` are exactly what mmap returned; nothing borrows the slice now.
        unsafe {
            libc::munmap(self.ptr, self.len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn mapped_and_read_bytes_are_identical() {
        let dir = std::env::temp_dir().join(format!("gtfs-feed-bytes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("feed.zip");
        let body: Vec<u8> = (0..100_000u32).flat_map(|i| i.to_le_bytes()).collect();
        std::fs::File::create(&path).unwrap().write_all(&body).unwrap();

        let feed = read_feed(&path).unwrap();
        #[cfg(unix)]
        assert!(matches!(feed, FeedBytes::Mapped(_)), "regular file is mapped on Unix");
        assert_eq!(&*feed, &body[..]);

        let empty = dir.join("empty.zip");
        std::fs::File::create(&empty).unwrap();
        assert!(matches!(read_feed(&empty).unwrap(), FeedBytes::Owned(ref b) if b.is_empty()));

        assert!(read_feed(&dir.join("missing.zip")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
