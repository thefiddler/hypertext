//! Compile-fail tests that require `feature = "datastar-js"`.

#![cfg(feature = "datastar-js")]

#[test]
fn datastar_js_compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/datastar-js-fail/*.rs");
}
