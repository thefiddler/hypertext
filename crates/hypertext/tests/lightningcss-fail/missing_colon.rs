// lightningcss catches missing colon between property and value,
// which cssparser alone would accept.
fn main() {
    let _ = hypertext::css! { .foo { color red } };
}
