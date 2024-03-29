use crate::ops::{BinaryOp, UnaryOp};
use crate::{Lit, Path, Span, Ty, TyKind};
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;

/// Expression in S-notation: 1 + 2 * 3 = (+ 1 (* 2 3))
/// Atoms is everything except Cons variant, pre-processed by pest.
#[derive(Clone, Eq, PartialEq)]
pub enum Expr {
    Call { method: Path, args: VecExpr },
    Index { object: Path, by: VecExpr },
    Lit(Lit),
    Tuple(VecExpr),
    Ty(Box<Ty>),
    Ref(Path),

    ConsU(UnaryOp, Box<Expr>),
    ConsB(BinaryOp, Box<(Expr, Expr)>),
}

impl Expr {
    pub fn unit() -> Expr {
        Expr::Ty(Box::new(Ty::new(TyKind::Unit)))
    }

    pub fn expect_ref(&self) -> Option<Path> {
        match self {
            Expr::Ref(path) => Some(path.clone()),
            _ => None,
        }
    }

    pub fn expect_call(&self) -> Option<(Path, VecExpr)> {
        match self {
            Expr::Call { method, args } => Some((method.clone(), args.clone())),
            _ => None,
        }
    }

    // pub fn expect_path(&self) -> Option<Path> {
    //     let mut path = Path::new();
    //     Self::expect_path_inner(self, &mut path)?;
    //     Some(path)
    // }
    //
    // fn expect_path_inner(expr: &Expr, path: &mut Path) -> Option<()> {
    //     match &expr {
    //         Expr::ConsB(op, cons) => {
    //             if *op == BinaryOp::Path {
    //                 Self::expect_path_inner(&cons.deref().0, path)?;
    //                 Self::expect_path_inner(&cons.deref().1, path)?;
    //                 Some(())
    //             } else {
    //                 None
    //             }
    //         }
    //         Expr::Ref(ident) => {
    //             path.append(ident.clone());
    //             Some(())
    //         }
    //         _ => None
    //     }
    // }

    pub fn format_kind(&self) -> String {
        match self {
            Expr::Call { .. } => "Call",
            Expr::Index { .. } => "Index",
            Expr::Lit(_) => "Lit",
            Expr::Tuple(_) => "Tuple",
            Expr::Ty(_) => "Ty",
            Expr::Ref(_) => "Ident",
            Expr::ConsU(_, _) => "Unary",
            Expr::ConsB(_, _) => "Binary",
        }
        .to_owned()
    }

    pub fn span(&self) -> Span {
        match self {
            Expr::Call { method, args } => method.span() + args.span(),
            Expr::Index { object, by } => object.span() + by.span(),
            Expr::Lit(lit) => lit.span.clone(),
            Expr::Tuple(t) => t.span(),
            Expr::Ty(ty) => ty.span.clone(),
            Expr::Ref(path) => path.span(),
            Expr::ConsU(_, cons) => cons.span(),
            Expr::ConsB(_, cons) => cons.deref().0.span() + cons.deref().1.span(),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct VecExpr(pub Vec<Expr>);

impl VecExpr {
    pub fn span(&self) -> Span {
        if self.0.is_empty() {
            panic!("VecExpr::span() called on empty");
        }
        self.0
            .iter()
            .skip(1)
            .fold(self.0[0].span(), |prev, expr| prev + expr.span())
    }
}

/// Expression that is eventually expected to be a literal
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TryEvaluateInto<F, T> {
    NotResolved(F),
    Resolved(T),
    Error,
}

impl Display for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Call { method, args } => {
                write!(f, "call:{}({})", method, args)
            }
            Expr::Index { object, by } => {
                write!(f, "index:{}({})", object, by)
            }
            Expr::Lit(lit) => {
                write!(f, "{}", lit)
            }
            Expr::Tuple(exprs) => {
                write!(f, "tuple({})", exprs)
            }
            Expr::Ty(ty) => {
                write!(f, "{}", ty)
            }
            Expr::Ref(ident) => {
                write!(f, "{}", ident)
            }
            Expr::ConsU(op, expr) => write!(f, "{}({})", op.to_str(), expr),
            Expr::ConsB(op, a) => {
                write!(f, "({} {} {})", op.to_str(), a.as_ref().0, a.as_ref().1)
            }
        }
    }
}

impl Display for VecExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        itertools::intersperse(
            self.0.iter().map(|expr| format!("{}", expr)),
            ", ".to_owned(),
        )
        .try_for_each(|s| write!(f, "{}", s))?;
        Ok(())
    }
}

impl<F: Display, T: Display> Display for TryEvaluateInto<F, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TryEvaluateInto::NotResolved(from) => write!(f, "NR({})", from),
            TryEvaluateInto::Resolved(to) => write!(f, "R({})", to),
            TryEvaluateInto::Error => write!(f, "ER()"),
        }
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Debug for VecExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}
