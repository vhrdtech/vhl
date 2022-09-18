use crate::dependencies::{Dependencies, Depends, ImportMerger, Package};
use crate::error::CodegenError;
use mtoken::{ToTokens, TokenStream};
use parser::span::Span;
use std::collections::HashSet;
use crate::Codegen;

/// Collection of code blocks with dependencies and source information.
///
/// Used to render a whole file for one target language while including all the requested dependencies
/// and packages exactly once.
pub struct File {
    pub code_pieces: Vec<(TokenStream, Dependencies, Span)>,
}

impl File {
    pub fn new() -> Self {
        File {
            code_pieces: vec![],
        }
    }

    /// Adds code piece into this file
    pub fn push<T: ToTokens + Depends>(&mut self, tokens: &T, origin: Span) {
        let mut ts = TokenStream::new();
        tokens.to_tokens(&mut ts);
        self.code_pieces.push((ts, tokens.dependencies(), origin));
    }

    /// Adds code piece into this file
    pub fn push_cg<T: Codegen<Error=CodegenError> + Depends>(&mut self, tokens: &T, origin: Span) -> Result<(), CodegenError> {
        let ts = tokens.codegen()?;
        self.code_pieces.push((ts, tokens.dependencies(), origin));
        Ok(())
    }

    pub fn render(&self) -> Result<(String, HashSet<Package>), CodegenError> {
        let mut depends_on = HashSet::new();
        let mut merger = ImportMerger::new();

        let mut code_pieces = String::new();
        for (ts, deps, source) in &self.code_pieces {
            // TODO: get target language and generate comments for it specifically
            code_pieces.push_str(format!("// Generated from {:#}\n", source).as_str());
            code_pieces.push_str(format!("{}\n\n", ts).as_str());
            for pkg in &deps.depends {
                depends_on.insert(pkg.clone());
            }
            merger.merge(&deps.uses);
        }

        let generator_comment = "// Generated by vhl...";
        let mut import_statements = String::new();
        import_statements.push_str(format!("{}\n", merger.render()).as_str());

        let whole_file = format!(
            "{}\n{}\n{}",
            generator_comment, import_statements, code_pieces
        );

        Ok((whole_file, depends_on))
    }
}
