// datastar-js catches invalid JS expressions in data-on:click
fn main() {
    let _ = hypertext::maud! { button data-on:click="if (" {} };
}
