//! Registry shape and uniqueness checks.

use std::collections::BTreeSet;

use super::*;

#[test]
fn names_are_unique_snake_case_and_actions_are_module_dot_verb() {
    let names: BTreeSet<&str> = COMMANDS.iter().map(|spec| spec.name).collect();
    assert_eq!(names.len(), COMMANDS.len());
    let snake =
        |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_lowercase() || b == b'_');
    for spec in COMMANDS {
        assert!(snake(spec.name), "{}", spec.name);
        let (module, verb) = spec.action.split_once('.').unwrap();
        assert!(snake(module) && snake(verb), "{}", spec.action);
    }
}
