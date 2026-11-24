//! Fuzz target for loading `settings.toml` (`docs/08` §2: `config::load`). Arbitrary bytes stand
//! for a corrupt or hostile file: parsing, migration and validation must return an error, never
//! panic, and anything that loads must save and load back to the same value.

#![no_main]

use ferrum_config::{SETTINGS_SCHEMA, Settings, store};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    let Ok((settings, _from)) = store::parse::<Settings>(text, &SETTINGS_SCHEMA) else { return };
    let saved = store::to_toml_string(&settings, &SETTINGS_SCHEMA).unwrap();
    let (reloaded, from) = store::parse::<Settings>(&saved, &SETTINGS_SCHEMA).unwrap();
    assert_eq!(from, SETTINGS_SCHEMA.current);
    assert_eq!(reloaded, settings);
});
