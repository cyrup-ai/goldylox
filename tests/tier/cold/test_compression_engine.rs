use goldylox::cache::tier::cold::compression_engine::*;

#[test]
fn test_gzip_roundtrip() {
    let engine = CompressionEngine::new(6);
    let original = b"Hello, world! ".repeat(1000);

    let compressed = engine
        .compress(&original, CompressionAlgorithm::Gzip)
        .expect("Gzip compression should succeed for test data");
    assert!(compressed.data.len() < original.len());

    let decompressed = engine.decompress(&compressed).expect("Gzip decompression should succeed for valid compressed data");
    assert_eq!(original, decompressed.as_slice());
}

#[test]
fn test_zstd_roundtrip() {
    let engine = CompressionEngine::new(3);
    let original = b"Test data for Zstd compression! ".repeat(500);

    let compressed = engine
        .compress(&original, CompressionAlgorithm::Zstd)
        .expect("Zstd compression should succeed for test data");
    assert!(compressed.data.len() < original.len());

    let decompressed = engine.decompress(&compressed).expect("Zstd decompression should succeed for valid compressed data");
    assert_eq!(original, decompressed.as_slice());
}

#[test]
fn test_snappy_roundtrip() {
    let engine = CompressionEngine::new(0);
    let original = b"Fast compression test data for Snappy! ".repeat(300);

    let compressed = engine
        .compress(&original, CompressionAlgorithm::Snappy)
        .expect("Snappy compression should succeed for test data");
    assert!(compressed.data.len() < original.len());

    let decompressed = engine.decompress(&compressed).expect("Snappy decompression should succeed for valid compressed data");
    assert_eq!(original, decompressed.as_slice());
}

#[test]
fn test_brotli_roundtrip() {
    let engine = CompressionEngine::new(6);
    let original = b"Brotli compression test data! ".repeat(400);

    let compressed = engine
        .compress(&original, CompressionAlgorithm::Brotli)
        .expect("Brotli compression should succeed for test data");
    assert!(compressed.data.len() < original.len());

    let decompressed = engine.decompress(&compressed).expect("Brotli decompression should succeed for valid compressed data");
    assert_eq!(original, decompressed.as_slice());
}

#[test]
fn test_integrity_verification() {
    let engine = CompressionEngine::new(3);
    let data = b"Test data for integrity verification";

    let mut compressed = engine.compress(data, CompressionAlgorithm::Zstd).expect("Zstd compression should succeed for integrity test data");

    // Corrupt the checksum
    compressed.checksum ^= 0xFFFFFFFF;

    match engine.decompress(&compressed) {
        Err(CompressionError::IntegrityCheckFailed { .. }) => {} // Expected
        _ => panic!("Should have detected integrity failure"),
    }
}

#[test]
fn test_adaptive_selection() {
    let engine = CompressionEngine::new(6);

    // Small data (512 bytes) returns current algorithm (Lz4)
    let small_data = vec![0u8; 512];
    assert_eq!(
        engine.select_algorithm(&small_data),
        CompressionAlgorithm::Lz4
    );

    // Large data should use Zstd
    let large_data = vec![0u8; 65536];
    assert_eq!(
        engine.select_algorithm(&large_data),
        CompressionAlgorithm::Zstd
    );
}

#[test]
fn test_all_algorithms() {
    let engine = CompressionEngine::new(6);
    let test_data = b"This is test data for all compression algorithms.".repeat(100);

    for algorithm in [
        CompressionAlgorithm::None,
        CompressionAlgorithm::Lz4,
        CompressionAlgorithm::Gzip,
        CompressionAlgorithm::Zstd,
        CompressionAlgorithm::Snappy,
        CompressionAlgorithm::Brotli,
    ] {
        let compressed = engine.compress(&test_data, algorithm).expect("Compression should succeed for all algorithms in test");
        let decompressed = engine.decompress(&compressed).expect("Decompression should succeed for all algorithms in test");
        assert_eq!(
            test_data,
            decompressed.as_slice(),
            "Failed for algorithm: {:?}",
            algorithm
        );
    }
}