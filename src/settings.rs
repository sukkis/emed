use config::Config;
use std::collections::HashMap;

/// Load editor settings from a TOML string, with defaults for missing keys.
pub fn load_settings(toml_content: &str) -> HashMap<String, String> {
    let settings = Config::builder()
        .set_default("theme", "pink")
        .unwrap()
        .set_default("tab_width", "4")
        .unwrap()
        .add_source(config::File::from_str(
            toml_content,
            config::FileFormat::Toml,
        ))
        .build()
        .unwrap();

    settings
        .try_deserialize::<HashMap<String, String>>()
        .unwrap()
}

#[cfg(test)]
#[test]
fn settings_file_returns_expected_values() {
    let settings = load_settings("theme = \"ocean\"\ntab_width = \"8\"\n");
    assert_eq!(settings.get("theme").unwrap(), "ocean");
    assert_eq!(settings.get("tab_width").unwrap(), "8");
}

#[test]
fn missing_settings_fall_back_to_defaults() {
    let settings = load_settings("");
    assert_eq!(settings.get("theme").unwrap(), "pink");
    assert_eq!(settings.get("tab_width").unwrap(), "4");
}

#[test]
fn partial_settings_merge_with_defaults() {
    let settings = load_settings("theme = \"ocean\"\n");
    assert_eq!(settings.get("theme").unwrap(), "ocean");
    assert_eq!(settings.get("tab_width").unwrap(), "4");
}
