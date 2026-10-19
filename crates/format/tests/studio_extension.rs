//! 7rrek saves documents as `.7rrek`, and the native-file check still accepts `.rsm`, `.vectorcraft`
//! and the legacy `.drawcraft` (the files File › Open and drag and drop route by it).
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (CLAUDE.md › Non-negotiables).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use vectorcraft_format::{STUDIO_EXTENSION, is_native_name};

#[test]
fn save_extension_is_7rrek_and_rsm_files_still_open() {
    assert_eq!(STUDIO_EXTENSION, "7rrek");
    assert!(is_native_name("art.7rrek"));
    assert!(is_native_name("art.RSM"), "rsm files still open");
    assert!(is_native_name("art.vectorcraft"));
    assert!(is_native_name("art.drawcraft"));
    assert!(!is_native_name("art.svg"));
}
