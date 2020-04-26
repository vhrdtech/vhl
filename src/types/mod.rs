#[derive(Debug)]
pub enum Numeric {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    In(u8),
    Un(u8),
    F16,
    F32,
    F64,
    Q(u8, u8), // "Q" notation
    UQ(u8, u8),
}

#[derive(Debug)]
pub enum Textual {
    Char,
    String,
    CChar,
    CString,
    UTF16String,
    UTF32String,
}

#[derive(Debug)]
pub enum Binary {}

#[derive(Debug)]
pub enum Sequence {
    Tuple,
    Array,
}

#[derive(Debug)]
pub struct User {
    //declaration: Statement?
}

#[derive(Debug)]
pub enum Type {
    Numeric(Numeric),
    Textual(Textual),
    Binary(Binary),
    Sequence(Sequence),
    User(User),
}