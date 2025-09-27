//! Unicode script conversion utilities for binary serialization
//!
//! This module provides efficient conversion between unicode_script::Script enum values
//! and compact u8 representations for binary storage in the cold tier cache.

use unicode_script::Script;

/// Convert unicode_script::Script to u8 for binary encoding
/// Uses a compact mapping that covers all common scripts
#[inline]
pub fn script_to_u8(script: Script) -> u8 {
    match script {
        Script::Common => 0,
        Script::Latin => 1,
        Script::Greek => 2,
        Script::Cyrillic => 3,
        Script::Armenian => 4,
        Script::Hebrew => 5,
        Script::Arabic => 6,
        Script::Syriac => 7,
        Script::Thaana => 8,
        Script::Devanagari => 9,
        Script::Bengali => 10,
        Script::Gurmukhi => 11,
        Script::Gujarati => 12,
        Script::Oriya => 13,
        Script::Tamil => 14,
        Script::Telugu => 15,
        Script::Kannada => 16,
        Script::Malayalam => 17,
        Script::Sinhala => 18,
        Script::Thai => 19,
        Script::Lao => 20,
        Script::Tibetan => 21,
        Script::Myanmar => 22,
        Script::Georgian => 23,
        Script::Hangul => 24,
        Script::Ethiopic => 25,
        Script::Cherokee => 26,
        Script::Canadian_Aboriginal => 27,
        Script::Ogham => 28,
        Script::Runic => 29,
        Script::Khmer => 30,
        Script::Mongolian => 31,
        Script::Hiragana => 32,
        Script::Katakana => 33,
        Script::Bopomofo => 34,
        Script::Han => 35,
        Script::Yi => 36,
        Script::Old_Italic => 37,
        Script::Gothic => 38,
        Script::Deseret => 39,
        Script::Inherited => 40,
        Script::Tagalog => 41,
        Script::Hanunoo => 42,
        Script::Buhid => 43,
        Script::Tagbanwa => 44,
        _ => 255, // Unknown/new scripts fall back to 255
    }
}

/// Convert u8 back to unicode_script::Script for binary decoding
/// Provides safe fallback to Script::Common for unknown values
#[inline]
pub fn script_from_u8(code: u8) -> Script {
    match code {
        0 => Script::Common,
        1 => Script::Latin,
        2 => Script::Greek,
        3 => Script::Cyrillic,
        4 => Script::Armenian,
        5 => Script::Hebrew,
        6 => Script::Arabic,
        7 => Script::Syriac,
        8 => Script::Thaana,
        9 => Script::Devanagari,
        10 => Script::Bengali,
        11 => Script::Gurmukhi,
        12 => Script::Gujarati,
        13 => Script::Oriya,
        14 => Script::Tamil,
        15 => Script::Telugu,
        16 => Script::Kannada,
        17 => Script::Malayalam,
        18 => Script::Sinhala,
        19 => Script::Thai,
        20 => Script::Lao,
        21 => Script::Tibetan,
        22 => Script::Myanmar,
        23 => Script::Georgian,
        24 => Script::Hangul,
        25 => Script::Ethiopic,
        26 => Script::Cherokee,
        27 => Script::Canadian_Aboriginal,
        28 => Script::Ogham,
        29 => Script::Runic,
        30 => Script::Khmer,
        31 => Script::Mongolian,
        32 => Script::Hiragana,
        33 => Script::Katakana,
        34 => Script::Bopomofo,
        35 => Script::Han,
        36 => Script::Yi,
        37 => Script::Old_Italic,
        38 => Script::Gothic,
        39 => Script::Deseret,
        40 => Script::Inherited,
        41 => Script::Tagalog,
        42 => Script::Hanunoo,
        43 => Script::Buhid,
        44 => Script::Tagbanwa,
        _ => Script::Common, // Unknown values default to Common
    }
}


