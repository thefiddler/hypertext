use proc_macro2::{Delimiter, LineColumn, Span, TokenStream, TokenTree};
use quote::quote;
use syn::LitStr;

use cssparser::{ToCss, Token, TokenSerializationType};

/// Convert a proc-macro token stream to minified CSS.
///
/// Phase 1: Reconstruct source-faithful CSS from token spans. Each token's
/// `Span::start()`/`Span::end()` positions are used to detect whether whitespace
/// existed between adjacent tokens in the original source. This correctly
/// preserves `.nav .item` (descendant selector) vs `.nav.active` (compound).
///
/// Phase 2: Minify the CSS using cssparser, stripping cosmetic whitespace
/// (around `{`, `:`, `;`, etc.) while preserving semantically significant spaces.
pub fn tokens_to_css(tokens: TokenStream) -> String {
    let source = source_css(tokens);
    if source.is_empty() {
        return source;
    }
    minify_css(&source)
}

// ---------------------------------------------------------------------------
// Phase 1: span-based source reconstruction
// ---------------------------------------------------------------------------

fn source_css(tokens: TokenStream) -> String {
    let mut out = String::new();
    let mut last_end: Option<LineColumn> = None;
    write_source_tokens(&mut out, &mut last_end, tokens);
    out
}

/// Returns true if there is a gap between two source positions (i.e., whitespace
/// existed between the previous token's end and the current token's start).
fn has_gap(end: LineColumn, start: LineColumn) -> bool {
    end.line != start.line || end.column < start.column
}

fn write_source_tokens(
    out: &mut String,
    last_end: &mut Option<LineColumn>,
    tokens: TokenStream,
) {
    for tt in tokens {
        let span = tt.span();
        let start = span.start();

        // Insert a space if there was whitespace between this token and the previous one.
        if let Some(end) = *last_end {
            if has_gap(end, start) {
                out.push(' ');
            }
        }

        match tt {
            TokenTree::Group(g) => {
                let delim = g.delimiter();
                let (open, close) = match delim {
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                out.push_str(open);

                // Position right after the opening delimiter.
                *last_end = if delim != Delimiter::None {
                    Some(LineColumn {
                        line: start.line,
                        column: start.column + 1,
                    })
                } else {
                    None
                };

                write_source_tokens(out, last_end, g.stream());

                // Check for space before the closing delimiter.
                let group_end = span.end();
                if delim != Delimiter::None {
                    let close_start = LineColumn {
                        line: group_end.line,
                        column: group_end.column.saturating_sub(1),
                    };
                    if let Some(end) = *last_end {
                        if has_gap(end, close_start) {
                            out.push(' ');
                        }
                    }
                }

                out.push_str(close);
                *last_end = Some(group_end);
            }
            TokenTree::Ident(ref i) => {
                out.push_str(&i.to_string());
                *last_end = Some(span.end());
            }
            TokenTree::Literal(ref l) => {
                out.push_str(&l.to_string());
                *last_end = Some(span.end());
            }
            TokenTree::Punct(ref p) => {
                out.push(p.as_char());
                *last_end = Some(span.end());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2: cssparser-based minification
// ---------------------------------------------------------------------------

/// Tracks the kind of token previously emitted, so we can decide whether
/// pending whitespace should be preserved or stripped.
#[derive(Clone, Copy)]
enum PrevCssToken {
    /// Start of block or file — whitespace after this is stripped.
    Nothing,
    /// After a structural delimiter (`: ; , { ( [ } ) ]`) — whitespace is stripped.
    Structural,
    /// After any other CSS token — whitespace may be significant.
    Regular(TokenSerializationType),
}

impl PrevCssToken {
    fn serialization_type(self) -> TokenSerializationType {
        match self {
            Self::Regular(st) => st,
            _ => TokenSerializationType::Nothing,
        }
    }
}

/// Returns true if whitespace before this token can be stripped.
fn is_strip_whitespace_before(token: &Token) -> bool {
    matches!(
        token,
        Token::CurlyBracketBlock
            | Token::CloseCurlyBracket
            | Token::ParenthesisBlock
            | Token::CloseParenthesis
            | Token::SquareBracketBlock
            | Token::CloseSquareBracket
            | Token::Colon
            | Token::Semicolon
            | Token::Comma
    )
}

/// Returns true if whitespace after the previous token can be stripped.
fn is_strip_whitespace_after(prev: PrevCssToken) -> bool {
    matches!(prev, PrevCssToken::Nothing | PrevCssToken::Structural)
}

fn minify_css(css: &str) -> String {
    let mut input = cssparser::ParserInput::new(css);
    let mut parser = cssparser::Parser::new(&mut input);
    let mut out = String::with_capacity(css.len());
    let mut prev = PrevCssToken::Nothing;
    minify_block(&mut parser, &mut out, &mut prev);
    out
}

fn minify_block(parser: &mut cssparser::Parser, out: &mut String, prev: &mut PrevCssToken) {
    let mut pending_whitespace = false;

    loop {
        let token = match parser.next_including_whitespace_and_comments() {
            Ok(t) => t.clone(),
            Err(_) => break,
        };

        match token {
            Token::WhiteSpace(_) | Token::Comment(_) => {
                pending_whitespace = true;
                continue;
            }
            _ => {}
        }

        let cur_ser = token.serialization_type();

        // Decide whether pending whitespace should be emitted.
        if pending_whitespace {
            let strip = is_strip_whitespace_before(&token) || is_strip_whitespace_after(*prev);
            if strip {
                // Even when stripping cosmetic whitespace, we must insert a separator
                // if adjacent tokens would otherwise merge into a different token.
                if prev.serialization_type().needs_separator_when_before(cur_ser) {
                    out.push(' ');
                }
            } else {
                out.push(' ');
            }
            pending_whitespace = false;
        } else if prev.serialization_type().needs_separator_when_before(cur_ser) {
            out.push(' ');
        }

        // Emit the token, recursing into nested blocks.
        match token {
            Token::CurlyBracketBlock => {
                out.push('{');
                let mut inner_prev = PrevCssToken::Nothing;
                let _ = parser.parse_nested_block(|inner| {
                    minify_block(inner, out, &mut inner_prev);
                    Ok::<_, cssparser::ParseError<()>>(())
                });
                out.push('}');
                *prev = PrevCssToken::Structural;
            }
            Token::ParenthesisBlock => {
                out.push('(');
                let mut inner_prev = PrevCssToken::Nothing;
                let _ = parser.parse_nested_block(|inner| {
                    minify_block(inner, out, &mut inner_prev);
                    Ok::<_, cssparser::ParseError<()>>(())
                });
                out.push(')');
                *prev = PrevCssToken::Structural;
            }
            Token::SquareBracketBlock => {
                out.push('[');
                let mut inner_prev = PrevCssToken::Nothing;
                let _ = parser.parse_nested_block(|inner| {
                    minify_block(inner, out, &mut inner_prev);
                    Ok::<_, cssparser::ParseError<()>>(())
                });
                out.push(']');
                *prev = PrevCssToken::Structural;
            }
            Token::Function(ref name) => {
                cssparser::serialize_identifier(name, &mut *out).unwrap();
                out.push('(');
                let mut inner_prev = PrevCssToken::Nothing;
                let _ = parser.parse_nested_block(|inner| {
                    minify_block(inner, out, &mut inner_prev);
                    Ok::<_, cssparser::ParseError<()>>(())
                });
                out.push(')');
                *prev = PrevCssToken::Structural;
            }
            Token::Colon | Token::Semicolon | Token::Comma => {
                token.to_css(&mut *out).unwrap();
                *prev = PrevCssToken::Structural;
            }
            ref other => {
                other.to_css(&mut *out).unwrap();
                *prev = PrevCssToken::Regular(cur_ser);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[cfg(feature = "lightningcss")]
pub fn validate_css(css: &str, span: Span) -> syn::Result<()> {
    use lightningcss::stylesheet::{ParserOptions, StyleSheet};

    StyleSheet::parse(css, ParserOptions::default())
        .map(|_| ())
        .map_err(|e| syn::Error::new(span, format!("CSS error: {e}")))
}

#[cfg(not(feature = "lightningcss"))]
pub fn validate_css(css: &str, span: Span) -> syn::Result<()> {
    let mut input = cssparser::ParserInput::new(css);
    let mut parser = cssparser::Parser::new(&mut input);

    exhaust(&mut parser)
        .map_err(|_| syn::Error::new(span, format!("invalid CSS: `{css}`")))
}

#[cfg(not(feature = "lightningcss"))]
fn exhaust<'i>(
    parser: &mut cssparser::Parser<'i, '_>,
) -> Result<(), cssparser::ParseError<'i, ()>> {
    loop {
        let is_nested = match parser.next() {
            Ok(token) => {
                if matches!(
                    token,
                    cssparser::Token::BadString(_) | cssparser::Token::BadUrl(_)
                ) {
                    return Err(parser.new_custom_error(()));
                }
                matches!(
                    token,
                    cssparser::Token::Function(_)
                        | cssparser::Token::CurlyBracketBlock
                        | cssparser::Token::ParenthesisBlock
                        | cssparser::Token::SquareBracketBlock
                )
            }
            Err(_) => return Ok(()),
        };
        if is_nested {
            parser.parse_nested_block(exhaust)?;
        }
    }
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

pub fn generate(tokens: TokenStream) -> syn::Result<TokenStream> {
    let css_string = tokens_to_css(tokens);
    validate_css(&css_string, Span::call_site())?;
    let lit = LitStr::new(&css_string, Span::mixed_site());
    Ok(quote! { ::hypertext::Raw::<_, ::hypertext::context::Node>::dangerously_create(#lit) })
}
