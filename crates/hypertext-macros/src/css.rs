use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use quote::quote;
use syn::LitStr;

#[derive(Clone, Copy)]
enum Prev {
    None,
    Word,
    Punct,
    Close,
}

pub fn tokens_to_css(tokens: TokenStream) -> String {
    let mut out = String::new();
    let mut prev = Prev::None;
    write_tokens(&mut out, &mut prev, tokens);
    out
}

fn write_tokens(out: &mut String, prev: &mut Prev, tokens: TokenStream) {
    for tt in tokens {
        match tt {
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                out.push_str(open);
                *prev = Prev::None;
                write_tokens(out, prev, g.stream());
                out.push_str(close);
                *prev = Prev::Close;
            }
            TokenTree::Ident(i) => {
                if matches!(prev, Prev::Word | Prev::Close) {
                    out.push(' ');
                }
                out.push_str(&i.to_string());
                *prev = Prev::Word;
            }
            TokenTree::Literal(l) => {
                if matches!(prev, Prev::Word | Prev::Close) {
                    out.push(' ');
                }
                out.push_str(&l.to_string());
                *prev = Prev::Word;
            }
            TokenTree::Punct(p) => {
                out.push(p.as_char());
                *prev = Prev::Punct;
            }
        }
    }
}

pub fn validate_css(css: &str, span: Span) -> syn::Result<()> {
    let mut input = cssparser::ParserInput::new(css);
    let mut parser = cssparser::Parser::new(&mut input);

    exhaust(&mut parser)
        .map_err(|_| syn::Error::new(span, format!("invalid CSS: `{css}`")))
}

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

pub fn generate(tokens: TokenStream) -> syn::Result<TokenStream> {
    let css_string = tokens_to_css(tokens);
    validate_css(&css_string, Span::call_site())?;
    let lit = LitStr::new(&css_string, Span::mixed_site());
    Ok(quote! { ::hypertext::Raw::<_, ::hypertext::context::Node>::dangerously_create(#lit) })
}
