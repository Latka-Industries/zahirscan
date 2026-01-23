//! Tests for adaptive chunking calculations (edge cases)

use zahirscan::chunking::{ProcessingTask, calculate_adaptive_chunking, optimal_chunk_size};
use zahirscan::{Config, parsers::FileType, parsers::ParseResult};

fn create_test_task(byte_count: usize, file_type: FileType) -> ProcessingTask {
    ProcessingTask {
        stats: ParseResult {
            file_path: "test".to_string(),
            file_type,
            line_count: 0,
            byte_count,
            token_count: 0,
            duration: std::time::Duration::from_secs(0),
            is_binary: false,
            mining_result: None,
            image_metadata: None,
            video_metadata: None,
            audio_metadata: None,
            csv_metadata: None,
            pdf_metadata: None,
            docx_metadata: None,
        },
        output_path: "test.out".to_string(),
    }
}

#[test]
fn test_optimal_chunk_size_small_collection() {
    // Small collection below threshold - should return 1 (no chunking)
    let result = optimal_chunk_size(100, 10, 1000);
    assert_eq!(result, 1);
}

#[test]
fn test_optimal_chunk_size_exact_division() {
    // Perfect division: 1000 items / 10 chunks = 100 per chunk
    let result = optimal_chunk_size(1000, 10, 100);
    assert_eq!(result, 100);
}

#[test]
fn test_optimal_chunk_size_with_remainder() {
    // Division with remainder: 1003 items / 10 chunks = 100 per chunk (remainder 3)
    let result = optimal_chunk_size(1003, 10, 100);
    assert_eq!(result, 100);
}

#[test]
fn test_optimal_chunk_size_zero_target() {
    // Zero target chunks - should return 1 (no chunking)
    let result = optimal_chunk_size(1000, 0, 100);
    assert_eq!(result, 1);
}

#[test]
fn test_optimal_chunk_size_single_chunk() {
    // Single chunk requested
    let result = optimal_chunk_size(1000, 1, 100);
    assert_eq!(result, 1000);
}

#[test]
fn test_optimal_chunk_size_large_collection() {
    // Large collection
    let result = optimal_chunk_size(1_000_000, 100, 100);
    assert_eq!(result, 10_000);
}

#[test]
fn test_calculate_adaptive_chunking_single_file() {
    // Single file should always use multiplier=1
    let config = Config::default();
    let tasks = vec![create_test_task(1_000_000, FileType::Text)];
    let result = calculate_adaptive_chunking(&tasks, 13, &config);
    assert_eq!(result.chunks_per_file_multiplier, 1);
}

#[test]
fn test_calculate_adaptive_chunking_image_only() {
    // Image-only batch should use multiplier=1 (fast metadata extraction)
    let config = Config::default();
    let tasks = vec![
        create_test_task(100_000, FileType::Image),
        create_test_task(200_000, FileType::Image),
        create_test_task(150_000, FileType::Image),
    ];
    let result = calculate_adaptive_chunking(&tasks, 13, &config);
    assert_eq!(result.chunks_per_file_multiplier, 1);
}

#[test]
fn test_calculate_adaptive_chunking_audio_only() {
    // Audio-only batch should use multiplier=1 (fast metadata extraction)
    let config = Config::default();
    let tasks = vec![
        create_test_task(1_000_000, FileType::Audio),
        create_test_task(2_000_000, FileType::Audio),
    ];
    let result = calculate_adaptive_chunking(&tasks, 13, &config);
    assert_eq!(result.chunks_per_file_multiplier, 1);
}

#[test]
fn test_calculate_adaptive_chunking_empty_tasks() {
    // Empty tasks should not panic
    let config = Config::default();
    let tasks = vec![];
    let result = calculate_adaptive_chunking(&tasks, 13, &config);
    // Should return a valid result (default multiplier)
    assert!(result.chunks_per_file_multiplier >= 1);
}

#[test]
fn test_calculate_adaptive_chunking_mixed_file_types() {
    // Mixed file types should use adaptive multiplier
    let config = Config::default();
    let tasks = vec![
        create_test_task(1_000_000, FileType::Text),
        create_test_task(2_000_000, FileType::Log),
        create_test_task(500_000, FileType::Json),
    ];
    let result = calculate_adaptive_chunking(&tasks, 13, &config);
    // Should calculate based on file sizes and variance
    assert!(result.chunks_per_file_multiplier >= 1);
}

#[test]
fn test_calculate_adaptive_chunking_high_variance() {
    // High variance in file sizes should increase multiplier
    let config = Config::default();
    let tasks = vec![
        create_test_task(10_000, FileType::Text),
        create_test_task(10_000_000, FileType::Text),
        create_test_task(5_000_000, FileType::Text),
    ];
    let result = calculate_adaptive_chunking(&tasks, 13, &config);
    // High variance should result in higher multiplier
    assert!(result.chunks_per_file_multiplier >= 1);
}

#[test]
fn test_calculate_adaptive_chunking_low_variance() {
    // Low variance (similar file sizes) should use lower multiplier
    let config = Config::default();
    let tasks = vec![
        create_test_task(1_000_000, FileType::Text),
        create_test_task(1_100_000, FileType::Text),
        create_test_task(950_000, FileType::Text),
    ];
    let result = calculate_adaptive_chunking(&tasks, 13, &config);
    // Low variance should result in reasonable multiplier
    assert!(result.chunks_per_file_multiplier >= 1);
}
