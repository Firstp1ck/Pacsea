//! Ignored release-mode measurements for the selected archive/hash dependency set.
//!
//! This is intentionally a deterministic integration measurement rather than a Criterion suite.
//! Wave 0 runs it under an external peak-RSS tool and records machine/toolchain metadata.

use std::fmt::Write as FmtWrite;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

/// Maximum expanded regular-file bytes approved for one source snapshot.
const MAX_EXPANDED_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum fully analyzable text bytes approved for one file.
const MAX_ANALYZABLE_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum approved archive entry count.
const MAX_ARCHIVE_ENTRIES: usize = 10_000;
/// Streaming buffer used by the measurement harness.
const BUFFER_BYTES: usize = 64 * 1024;

/// One timing observation printed by the benchmark harness.
struct Measurement {
    /// Stable case name.
    name: &'static str,
    /// Bytes decoded, iterated, or hashed.
    bytes: u64,
    /// Elapsed wall time.
    elapsed: Duration,
}

impl Measurement {
    /// What: Print one stable machine-readable-ish benchmark line.
    ///
    /// Inputs: None beyond the captured measurement.
    ///
    /// Output:
    /// - A `PI_SCAN_BENCH` line containing bytes, seconds, and MiB/s.
    ///
    /// Details:
    /// - Peak RSS is captured by the external runner rather than sampled in-process.
    fn print(&self) {
        let seconds = self.elapsed.as_secs_f64().max(f64::EPSILON);
        // Measurement inputs are capped at 256 MiB, so this conversion is exact in `f64`.
        #[allow(clippy::cast_precision_loss)]
        let mib_per_second = (self.bytes as f64 / (1024.0 * 1024.0)) / seconds;
        println!(
            "PI_SCAN_BENCH name={} bytes={} seconds={seconds:.6} mib_per_second={mib_per_second:.3}",
            self.name, self.bytes
        );
    }
}

/// What: Stream bytes through SHA-256 and BLAKE2b-512 without retaining the body.
///
/// Inputs:
/// - `reader`: Decoded or raw byte stream.
///
/// Output:
/// - Total bytes plus both digest byte vectors.
///
/// Details:
/// - Uses a fixed 64 KiB buffer and algorithm-specific traits because selected stable crates use
///   two digest-trait generations.
fn copy_and_hash<R: Read>(mut reader: R) -> std::io::Result<(u64, Vec<u8>, Vec<u8>)> {
    use blake2::Digest as Blake2Digest;
    use sha2::Digest as Sha2Digest;

    let mut sha256 = sha2::Sha256::new();
    let mut blake2 = blake2::Blake2b512::new();
    let mut buffer = vec![0_u8; BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        Sha2Digest::update(&mut sha256, &buffer[..read]);
        Blake2Digest::update(&mut blake2, &buffer[..read]);
        total += read as u64;
    }
    Ok((
        total,
        Sha2Digest::finalize(sha256).to_vec(),
        Blake2Digest::finalize(blake2).to_vec(),
    ))
}

/// What: Measure one streaming reader and validate its decoded byte count.
///
/// Inputs:
/// - `name`: Stable case label.
/// - `reader`: Input/decoder stream.
/// - `expected_bytes`: Required decoded length.
///
/// Output:
/// - Timing measurement or an I/O/contract error.
///
/// Details:
/// - Hashes the stream so the optimizer cannot elide decoding work.
fn measure_reader<R: Read>(
    name: &'static str,
    reader: R,
    expected_bytes: u64,
) -> Result<Measurement, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let (bytes, sha256, blake2) = copy_and_hash(reader)?;
    if bytes != expected_bytes {
        return Err(format!("{name} decoded {bytes} bytes, expected {expected_bytes}").into());
    }
    std::hint::black_box((sha256, blake2));
    Ok(Measurement {
        name,
        bytes,
        elapsed: started.elapsed(),
    })
}

/// What: Print the byte length and SHA-256 identity of one generated compressed fixture.
///
/// Inputs:
/// - `name`: Stable fixture label.
/// - `bytes`: Complete compressed fixture bytes.
///
/// Output:
/// - A reproducible `PI_SCAN_FIXTURE` evidence line.
///
/// Details:
/// - Fixture generation and hashing occur outside timed decoder measurements.
fn print_fixture(name: &str, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let (size, sha256, blake2) = copy_and_hash(bytes)?;
    std::hint::black_box(blake2);
    let mut digest = String::with_capacity(sha256.len() * 2);
    for byte in sha256 {
        write!(&mut digest, "{byte:02x}")?;
    }
    println!("PI_SCAN_FIXTURE name={name} bytes={size} sha256={digest}");
    Ok(())
}

/// What: Encode a repeated-byte stream as gzip without allocating the expanded body.
///
/// Inputs:
/// - `bytes`: Expanded byte count.
///
/// Output:
/// - Compressed gzip bytes.
///
/// Details:
/// - Fixture generation is outside the timed decoder measurement.
fn gzip_fixture(bytes: u64) -> std::io::Result<Vec<u8>> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    std::io::copy(&mut std::io::repeat(0x61).take(bytes), &mut encoder)?;
    encoder.finish()
}

/// What: Encode a repeated-byte stream as bzip2 without allocating the expanded body.
///
/// Inputs:
/// - `bytes`: Expanded byte count.
///
/// Output:
/// - Compressed bzip2 bytes.
///
/// Details:
/// - Uses the selected Rust default backend.
fn bzip2_fixture(bytes: u64) -> std::io::Result<Vec<u8>> {
    let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
    std::io::copy(&mut std::io::repeat(0x62).take(bytes), &mut encoder)?;
    encoder.finish()
}

/// What: Encode a repeated-byte stream as Zstandard without allocating the expanded body.
///
/// Inputs:
/// - `bytes`: Expanded byte count.
///
/// Output:
/// - Compressed Zstandard bytes.
///
/// Details:
/// - Fixture generation uses compression level one and is outside timed decoding.
fn zstd_fixture(bytes: u64) -> std::io::Result<Vec<u8>> {
    let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 1)?;
    std::io::copy(&mut std::io::repeat(0x63).take(bytes), &mut encoder)?;
    encoder.finish()
}

/// What: Encode a repeated-byte stream as XZ without external process state.
///
/// Inputs:
/// - `bytes`: Expanded byte count.
///
/// Output:
/// - Deterministic compressed XZ bytes.
///
/// Details:
/// - Uses the selected crate's test-only `encoder` feature at preset one; production builds keep
///   only `std,xz` decoder features.
fn xz_fixture(bytes: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let options = lzma_rust2::XzOptions::with_preset(1);
    let mut encoder = lzma_rust2::XzWriter::new(Vec::new(), options)?;
    std::io::copy(&mut std::io::repeat(0x64).take(bytes), &mut encoder)?;
    Ok(encoder.finish()?)
}

/// What: Generate a gzip-compressed tar fixture at the approved entry-count maximum.
///
/// Inputs: None.
///
/// Output:
/// - Compressed tar bytes containing 10,000 one-byte regular files.
///
/// Details:
/// - Uses tar writing only for deterministic benchmark fixture generation.
fn tar_fixture() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);
    for index in 0..MAX_ARCHIVE_ENTRIES {
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(
            &mut header,
            format!("src/file-{index:05}.txt"),
            std::io::repeat(0x65).take(1),
        )?;
    }
    builder.finish()?;
    let encoder = builder.into_inner()?;
    Ok(encoder.finish()?)
}

/// What: Measure entry-by-entry tar iteration at the approved entry-count maximum.
///
/// Inputs:
/// - `compressed`: Gzip-compressed tar fixture.
///
/// Output:
/// - Timing measurement covering all 10,000 entries.
///
/// Details:
/// - Reads every entry body and never invokes an extraction helper.
fn measure_tar(compressed: &[u8]) -> Result<Measurement, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let decoder = flate2::read::MultiGzDecoder::new(compressed);
    let mut archive = tar::Archive::new(decoder);
    let mut entries = 0_usize;
    let mut bytes = 0_u64;
    for entry in archive.entries()? {
        let mut entry = entry?;
        bytes += std::io::copy(&mut entry, &mut std::io::sink())?;
        entries += 1;
    }
    if entries != MAX_ARCHIVE_ENTRIES {
        return Err(format!("tar iterated {entries} entries").into());
    }
    Ok(Measurement {
        name: "tar_gzip_10000_entries",
        bytes,
        elapsed: started.elapsed(),
    })
}

/// What: Generate and measure a Stored ZIP at the approved entry-count maximum.
///
/// Inputs: None.
///
/// Output:
/// - Timing measurement covering all 10,000 entries.
///
/// Details:
/// - Uses by-index reads and never invokes ZIP extraction helpers.
fn measure_zip() -> Result<Measurement, Box<dyn std::error::Error>> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for index in 0..MAX_ARCHIVE_ENTRIES {
        writer.start_file(format!("src/file-{index:05}.txt"), options)?;
        writer.write_all(&[0x66])?;
    }
    let bytes = writer.finish()?.into_inner();
    print_fixture("zip_stored_10000_entries", &bytes)?;

    let started = Instant::now();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut decoded = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        decoded += std::io::copy(&mut entry, &mut std::io::sink())?;
    }
    if archive.len() != MAX_ARCHIVE_ENTRIES {
        return Err(format!("zip iterated {} entries", archive.len()).into());
    }
    Ok(Measurement {
        name: "zip_stored_10000_entries",
        bytes: decoded,
        elapsed: started.elapsed(),
    })
}

/// What: Benchmark selected decoders, dual hashing, and archive entry iteration at approved bounds.
///
/// Inputs:
/// - Deterministic fixtures generated entirely through the selected Rust dependencies.
///
/// Output:
/// - Stable timing lines; external runner records peak RSS and environment metadata.
///
/// Details:
/// - Ignored in normal CI because it intentionally processes hundreds of MiB in release mode.
#[test]
#[ignore = "Wave 0 release benchmark; run explicitly with external peak-RSS measurement"]
fn wave0_dependency_benchmark_at_approved_bounds() -> Result<(), Box<dyn std::error::Error>> {
    let raw_started = Instant::now();
    let (bytes, sha256, blake2) =
        copy_and_hash(std::io::repeat(0x67).take(MAX_ANALYZABLE_FILE_BYTES))?;
    std::hint::black_box((sha256, blake2));
    Measurement {
        name: "dual_hash_16mib",
        bytes,
        elapsed: raw_started.elapsed(),
    }
    .print();

    let gzip = gzip_fixture(MAX_EXPANDED_BYTES)?;
    print_fixture("gzip_256mib", &gzip)?;
    measure_reader(
        "gzip_decode_256mib",
        flate2::read::MultiGzDecoder::new(gzip.as_slice()),
        MAX_EXPANDED_BYTES,
    )?
    .print();

    let bzip2 = bzip2_fixture(MAX_ANALYZABLE_FILE_BYTES)?;
    print_fixture("bzip2_16mib", &bzip2)?;
    measure_reader(
        "bzip2_decode_16mib",
        bzip2::read::MultiBzDecoder::new(bzip2.as_slice()),
        MAX_ANALYZABLE_FILE_BYTES,
    )?
    .print();

    let zstd = zstd_fixture(MAX_ANALYZABLE_FILE_BYTES)?;
    print_fixture("zstd_16mib", &zstd)?;
    measure_reader(
        "zstd_decode_16mib",
        zstd::stream::read::Decoder::new(zstd.as_slice())?,
        MAX_ANALYZABLE_FILE_BYTES,
    )?
    .print();

    let xz = xz_fixture(MAX_ANALYZABLE_FILE_BYTES)?;
    print_fixture("xz_16mib", &xz)?;
    measure_reader(
        "xz_decode_16mib",
        lzma_rust2::XzReader::new(xz.as_slice(), true),
        MAX_ANALYZABLE_FILE_BYTES,
    )?
    .print();

    let tar = tar_fixture()?;
    print_fixture("tar_gzip_10000_entries", &tar)?;
    measure_tar(&tar)?.print();
    measure_zip()?.print();
    Ok(())
}
