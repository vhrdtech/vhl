use proc_macro2::{TokenStream};
use quote::{quote, TokenStreamExt, ToTokens};
use parser::ast::item_tuple::TupleFields;
use std::marker::PhantomData;
use crate::rust::ty::CGTy;

pub struct CGTupleFields<'i, 'c> {
    pub inner: &'c TupleFields,
    pub _p: &'i PhantomData<()>
}

impl<'i, 'c> ToTokens for CGTupleFields<'i, 'c> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let fields = self.inner.fields.iter().map(
            |i| CGTy {
                inner: &i,
                _p: &PhantomData
            }
        );
        tokens.append_all(quote! {
            ( #(#fields),* )
        });
    }
}