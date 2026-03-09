// datastar-js catches invalid JS expressions in data-show
fn main() {
    let _ = hypertext::maud! { div data-show="{{" {} };
}
