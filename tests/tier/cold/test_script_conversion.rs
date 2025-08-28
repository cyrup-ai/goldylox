use goldylox::cache::tier::cold::script_conversion::*;

#[test]
fn test_script_roundtrip() {
    let scripts = [
        Script::Common,
        Script::Latin,
        Script::Arabic,
        Script::Han,
        Script::Hiragana,
        Script::Katakana,
    ];

    for script in scripts {
        let encoded = script_to_u8(script);
        let decoded = script_from_u8(encoded);
        assert_eq!(script, decoded);
    }
}

#[test]
fn test_unknown_script_fallback() {
    let unknown_code = 200u8;
    let decoded = script_from_u8(unknown_code);
    assert_eq!(decoded, Script::Common);
}