//! Compile-fail tests that require `feature = "lightningcss"`.

#![cfg(feature = "lightningcss")]

#[test]
fn lightningcss_compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/lightningcss-fail/*.rs");
}
