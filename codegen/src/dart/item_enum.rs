use proc_macro2::{TokenStream, };
use quote::{quote, TokenStreamExt, ToTokens};
use parser::ast::item_enum::{ItemEnum};
use crate::dart::docs::CGDocs;

pub struct CGItemEnum<'i, 'c> {
    pub docs: CGDocs<'i, 'c>,
    // pub typename: CGTypename<'i, 'c>,
    // pub items: &'c EnumItems<'i>,
}

impl<'i, 'c> CGItemEnum<'i, 'c> {
    pub fn new(item_enum: &'c ItemEnum<'i>) -> Self {
        Self {
            docs: CGDocs { inner: &item_enum.docs },
            // typename: CGTypename { inner: &item_enum.typename },
            // items: &item_enum.items
        }
    }
}

impl<'i, 'c> ToTokens for CGItemEnum<'i, 'c> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let docs = &self.docs;
        // let typename = &self.typename;
        // let items = self.items.items.iter().map(
        //     |i| CGEnumItem {
        //         docs: CGDocs { inner: &i.docs },
        //         name: CGEnumItemName { inner: &i.name },
        //         kind: CGEnumItemKind { inner: &i.kind }
        //     }
        // );
        tokens.append_all(quote! {
            #docs
        });
    }
}