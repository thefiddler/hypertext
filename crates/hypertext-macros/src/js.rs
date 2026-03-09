#[cfg(feature = "datastar-js")]
use proc_macro2::Span;

/// Classifies what kind of JS value a Datastar attribute expects.
#[cfg(feature = "datastar-js")]
pub enum JsValueKind {
    /// A JavaScript expression (e.g., `$count++`, `$isVisible`).
    Expression,
    /// A JavaScript object literal (e.g., `{count: 0}`).
    ObjectLiteral,
    /// No JS validation needed (signal names, no-value attrs, etc.).
    None,
}

/// Determine the expected JS value kind from the `rest` portion of a `data-*`
/// attribute name (the part after `data-`).
#[cfg(feature = "datastar-js")]
pub fn datastar_value_kind(rest: &str) -> JsValueKind {
    match rest {
        "show" | "text" | "init" | "effect" | "on-interval" | "on-intersect"
        | "on-signal-patch" => JsValueKind::Expression,
        "signals" => JsValueKind::ObjectLiteral,
        s if s.starts_with("on:")
            || s.starts_with("computed:")
            || s.starts_with("attr:")
            || s.starts_with("class:")
            || s.starts_with("style:") =>
        {
            JsValueKind::Expression
        }
        _ => JsValueKind::None,
    }
}

#[cfg(feature = "datastar-js")]
pub fn validate_js_expression(js: &str, span: Span) -> syn::Result<()> {
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    let allocator = Allocator::default();
    // Wrap as `0,(EXPR)` — the comma operator forces expression context
    let wrapped = format!("0,({js})");
    let ret = Parser::new(&allocator, &wrapped, SourceType::mjs()).parse();

    if ret.errors.is_empty() {
        Ok(())
    } else {
        let msgs: Vec<String> = ret.errors.iter().map(|e| e.to_string()).collect();
        Err(syn::Error::new(
            span,
            format!("invalid JS expression: {}", msgs.join("; ")),
        ))
    }
}

#[cfg(feature = "datastar-js")]
pub fn validate_js_object(js: &str, span: Span) -> syn::Result<()> {
    // Objects are wrapped in parens to disambiguate from blocks
    validate_js_expression(js, span)
        .map_err(|_| syn::Error::new(span, format!("invalid JS object literal: `{js}`")))
}
