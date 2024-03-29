use ast::Identifier;
use mtoken::ext::TokenStreamExt;
use mtoken::token::IdentFlavor;
use mtoken::{Ident, ToTokens, TokenStream};
use std::rc::Rc;

#[derive(Clone)]
pub struct CGIdentifier<'ast> {
    pub inner: &'ast Identifier,
}

impl<'ast> ToTokens for CGIdentifier<'ast> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append(Ident::new(
            Rc::clone(&self.inner.symbols),
            IdentFlavor::RustAutoRaw,
        ));
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ast::{IdentifierContext, Span};
    use mquote::mquote;

    #[test]
    fn identifier() {
        let ast_ident = Identifier {
            symbols: Rc::new("value".to_string()),
            context: IdentifierContext::VariableRefName,
            span: Span::call_site(),
        };
        let cg_ident = CGIdentifier { inner: &ast_ident };
        let mut ts = TokenStream::new();
        cg_ident.to_tokens(&mut ts);
        assert_eq!(format!("{}", ts), "value");
    }

    #[test]
    fn identifier_via_mquote() {
        let ast_ident = Identifier {
            symbols: Rc::new("value".to_string()),
            context: IdentifierContext::VariableRefName,
            span: Span::call_site(),
        };
        let cg_ident = CGIdentifier { inner: &ast_ident };
        let ts = mquote!(rust r#"
            Λcg_ident
        "#);
        assert_eq!(format!("{}", ts), "value");
    }

    #[test]
    fn identifier_autoraw_mquote() {
        let ts = mquote!(rust r#"
            type
        "#);
        assert_eq!(format!("{}", ts), "r#type");
    }
}
