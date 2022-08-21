use crate::ast::ty::Ty;
use super::prelude::*;

#[derive(Debug)]
pub struct DefTypeAlias<'i> {
    pub doc: Doc<'i>,
    pub attrs: Attrs<'i>,
    pub typename: Typename<'i>,
    pub r#type: Ty<'i>,
}

impl<'i> Parse<'i> for DefTypeAlias<'i> {
    fn parse<'m>(input: &mut ParseInput<'i, 'm>) -> Result<Self, ParseErrorSource> {
        let mut input = ParseInput::fork(input.expect1(Rule::type_alias_def)?, input);
        Ok(DefTypeAlias {
            doc: input.parse()?,
            attrs: input.parse()?,
            typename: input.parse()?,
            r#type: input.parse()?
        })
    }
}