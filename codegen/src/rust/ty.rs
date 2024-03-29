use crate::prelude::*;
use ast::{Ty, TyKind};

#[derive(Clone)]
pub struct CGTy<'ast> {
    pub inner: &'ast Ty,
}

impl<'ast> ToTokens for CGTy<'ast> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match &self.inner.kind {
            TyKind::Unit => {
                tokens.append_all(mquote!(rust r#"
                    ()
                "#));
            }
            TyKind::Boolean => {
                tokens.append(mtoken::Ident::new(
                    Rc::new("bool".to_string()),
                    IdentFlavor::Plain,
                ));
            }
            TyKind::Discrete(discrete) => {
                let is_signed = if is_native_discrete(discrete.bits) {
                    if discrete.is_signed {
                        "i"
                    } else {
                        "u"
                    }
                } else if discrete.is_signed {
                    "VI"
                } else {
                    "VU"
                };
                let discrete = format!("{}{}", is_signed, discrete.bits);
                tokens.append_all(mquote!(rust r#"
                    Λdiscrete
                "#));
            }
            TyKind::Float(float) => {
                tokens.append_all(mquote!(rust r#"
                    f◡Λ{float.bits}
                "#));
            }
            TyKind::Array { ty, len_bound: _ } => {
                println!("tycg attrs: {:?}", self.inner.attrs);
                let ty = CGTy { inner: ty };
                tokens.append_all(mquote!(rust r#"
                    [Λty; todo]
                "#));
            }
            kind => unimplemented!("{:?}", kind),
        }
    }
}

fn is_native_discrete(bits: u32) -> bool {
    bits == 8 || bits == 16 || bits == 32 || bits == 64 || bits == 128
}
