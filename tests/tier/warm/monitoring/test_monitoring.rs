use goldylox::cache::tier::warm::monitoring::*;

#[test]
fn test_pressure_level_checks() {
    assert!(is_critical_pressure(0.96));
    assert!(!is_critical_pressure(0.94));

    assert!(is_high_pressure(0.86));
    assert!(!is_high_pressure(0.84));

    assert!(is_medium_pressure(0.71));
    assert!(!is_medium_pressure(0.69));

    assert!(is_low_pressure(0.51));
    assert!(!is_low_pressure(0.49));
}

#[test]
fn test_efficiency_score_calculation() {
    let score = calculate_efficiency_score(512, 1024, 0.9);
    assert!(score > 0.5 && score < 1.0);
}

#[test]
fn test_memory_size_formatting() {
    assert_eq!(format_memory_size(1024), "1.00 KB");
    assert_eq!(format_memory_size(1048576), "1.00 MB");
    assert_eq!(format_memory_size(1073741824), "1.00 GB");
}

#[test]
fn test_oom_time_estimation() {
    let time = estimate_oom_time(512, 1024, 10.0);
    assert_eq!(time, Some(51.2));

    let no_time = estimate_oom_time(1024, 1024, 10.0);
    assert_eq!(no_time, None);
}