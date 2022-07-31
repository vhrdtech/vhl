use parser::ast::doc::Doc;
use mtoken::{ToTokens, TokenStream, Span, Comment, CommentFlavor, ext::TokenStreamExt};
use mquote::mquote;

pub struct CGDocs<'i, 'c> {
    pub inner: &'c Doc<'i>,
    pub flavor: CommentFlavor,
}

impl<'i, 'c> ToTokens for CGDocs<'i, 'c> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let lines = self.inner.lines.iter().map(|line|
            Comment::new(line, self.flavor, Span::call_site())
        );
        tokens.append_all(mquote!(rust r#"
            #(#lines)*
        "#));
    }
}