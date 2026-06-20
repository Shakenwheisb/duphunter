//! Streaming content hashing.
//!
//! Files are always read in bounded chunks — we never load a whole file into
//! memory. Two granularities are exposed:
//!
//! * [`partial_hash`] — cheap pre-filter: samples the head, and for large files
//!   also the middle and tail. Eliminates same-size files that differ near an
//!   end without paying to read the entire file.
//! * [`full_hash`] — definitive: streams the entire file through the chosen
//!   algorithm.

use crate::error::Result;
use crate::model::HashAlgo;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Read buffer size (1 MiB) — balances syscall overhead against memory use.
const CHUNK: usize = 1024 * 1024;
/// Head/middle/tail sample window for partial hashing (64 KiB).
const SAMPLE: usize = 64 * 1024;
/// Files larger than this also get middle+tail samples in the partial hash.
const LARGE_FILE: u64 = 256 * 1024;

/// Incremental hasher that dispatches to the configured algorithm.
enum Hasher {
    Blake3(Box<blake3::Hasher>),
    Xxh3(xxhash_rust::xxh3::Xxh3),
    Sha256(sha2::Sha256),
}

impl Hasher {
    fn new(algo: HashAlgo) -> Self {
        match algo {
            HashAlgo::Blake3 => Hasher::Blake3(Box::new(blake3::Hasher::new())),
            HashAlgo::Xxh3 => Hasher::Xxh3(xxhash_rust::xxh3::Xxh3::new()),
            HashAlgo::Sha256 => {
                use sha2::Digest;
                Hasher::Sha256(sha2::Sha256::new())
            }
        }
    }

    fn update(&mut self, data: &[u8]) {
        match self {
            Hasher::Blake3(h) => {
                h.update(data);
            }
            Hasher::Xxh3(h) => h.update(data),
            Hasher::Sha256(h) => {
                use sha2::Digest;
                h.update(data);
            }
        }
    }

    fn finalize_hex(self) -> String {
        match self {
            Hasher::Blake3(h) => h.finalize().to_hex().to_string(),
            Hasher::Xxh3(h) => format!("{:016x}", h.digest()),
            Hasher::Sha256(h) => {
                use sha2::Digest;
                hex_encode(&h.finalize())
            }
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Cheap pre-filter hash. For small files this reads the whole file (so it is
/// already definitive); for large files it samples head+middle+tail plus the
/// exact size, which is enough to separate most non-duplicates cheaply.
pub fn partial_hash(path: &Path, size: u64, algo: HashAlgo) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Hasher::new(algo);
    // Mix the size in so different-size files can never share a partial hash.
    hasher.update(&size.to_le_bytes());

    let mut buf = vec![0u8; SAMPLE];

    // Head sample.
    let n = read_window(&mut file, &mut buf)?;
    hasher.update(&buf[..n]);

    if size > LARGE_FILE {
        // Middle sample.
        let mid = size / 2;
        file.seek(SeekFrom::Start(mid))?;
        let n = read_window(&mut file, &mut buf)?;
        hasher.update(&buf[..n]);

        // Tail sample.
        let tail = size.saturating_sub(SAMPLE as u64);
        file.seek(SeekFrom::Start(tail))?;
        let n = read_window(&mut file, &mut buf)?;
        hasher.update(&buf[..n]);
    }

    Ok(hasher.finalize_hex())
}

/// Definitive whole-file streaming hash.
pub fn full_hash(path: &Path, algo: HashAlgo) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Hasher::new(algo);
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize_hex())
}

/// Read up to `buf.len()` bytes, tolerating short reads.
fn read_window(file: &mut File, buf: &mut [u8]) -> Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = file.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

/// Streaming byte-for-byte equality of two files. Used by paranoid mode to rule
/// out the rare hash collision before treating files as identical. Returns true
/// only if every byte matches.
pub fn bytes_equal(a: &Path, b: &Path) -> Result<bool> {
    let mut fa = File::open(a)?;
    let mut fb = File::open(b)?;
    let mut ba = vec![0u8; CHUNK];
    let mut bb = vec![0u8; CHUNK];
    loop {
        let na = read_window(&mut fa, &mut ba)?;
        let nb = read_window(&mut fb, &mut bb)?;
        if na != nb {
            return Ok(false);
        }
        if na == 0 {
            return Ok(true);
        }
        if ba[..na] != bb[..nb] {
            return Ok(false);
        }
    }
}
