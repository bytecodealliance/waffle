//! Displaying IR.

use super::{FuncDecl, FunctionBody, Module, SourceLoc, ValueDef};
use crate::entity::EntityRef;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter, Result as FmtResult};

/// Hooks to print information after instruction, before and after blocks
/// and before and after functions.
pub trait PrintDecorator {
    /// Print arbitrary text after an instruction.
    ///
    /// Invoked after every instruction in a block. The instruction has already been printed on its own line;
    /// this method can print content after the operator, if desired.
    fn after_inst(&self, _value: super::Value, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }

    /// Print arbitrary text before the body of a block.
    ///
    /// Invoked before the block body. The block id and parameters have already been printed on its own line;
    /// this method can print content on the line below the block id, before the body of the block is printed, if desired.
    fn before_block(&self, _block: super::Block, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }

    /// Print arbitrary text after the body of a block.
    ///
    /// Invoked after the block body, before the terminator. The block body has already been printed on its own line(s);
    /// this method can print content on the line after the last instruction in the block body, before the terminator is printed.
    fn after_block(&self, _block: super::Block, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }

    /// Print arbitrary text before the body of a function.
    ///
    /// Invoked before the function body is printed. The function id and signature have already been printed on its own line;
    /// this method can print content on the line before the function signature line, before the function body is printed.
    fn before_function_body(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }

    /// Print arbitrary text after the body of a function.
    ///
    /// Invoked after the function body is printed. The function body has already been printed;
    /// this method can print content on the line after the return block of the function, before the last curly brace to end the function is printed.
    fn after_function_body(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct NOPPrintDecorator();
impl PrintDecorator for NOPPrintDecorator {}

/// A wrapper around a `FunctionBody` together with some auxiliary
/// information to perform a pretty-print of that function.
pub struct FunctionBodyDisplay<'a, PD: PrintDecorator> {
    pub(crate) body: &'a FunctionBody,
    pub(crate) indent: &'a str,
    pub(crate) verbose: bool,
    pub(crate) module: Option<&'a Module<'a>>,
    pub(crate) decorator: Option<&'a PD>,
}

impl<'a, PD: PrintDecorator> Display for FunctionBodyDisplay<'a, PD> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let arg_tys = self
            .body
            .locals
            .values()
            .take(self.body.n_params)
            .map(|&ty| format!("{}", ty))
            .collect::<Vec<_>>();
        let ret_tys = self
            .body
            .rets
            .iter()
            .map(|&ty| format!("{}", ty))
            .collect::<Vec<_>>();
        writeln!(
            f,
            "{}function({}) -> {} {{",
            self.indent,
            arg_tys.join(", "),
            ret_tys.join(", ")
        )?;

        if let Some(decorator) = self.decorator {
            decorator.before_function_body(f)?;
        }

        for (value, value_def) in self.body.values.entries() {
            match value_def {
                ValueDef::Operator(op, args, tys) if self.verbose => {
                    writeln!(
                        f,
                        "{}    {} = {} {} # {} ",
                        self.indent,
                        value,
                        op,
                        self.body.arg_pool[*args]
                            .iter()
                            .map(|arg| format!("{}", arg))
                            .collect::<Vec<_>>()
                            .join(", "),
                        self.body.type_pool[*tys]
                            .iter()
                            .map(|arg| format!("{}", arg))
                            .collect::<Vec<_>>()
                            .join(", "),
                    )?;
                }
                ValueDef::BlockParam(block, idx, ty) if self.verbose => writeln!(
                    f,
                    "{}    {} = blockparam {}, {} # {}",
                    self.indent, value, block, idx, ty
                )?,
                ValueDef::Alias(alias_target) => {
                    if self.verbose {
                        writeln!(f, "{}    {} = {}", self.indent, value, alias_target)?
                    }
                }
                ValueDef::PickOutput(val, idx, ty) => {
                    writeln!(f, "{}    {} = {}.{} # {}", self.indent, value, val, idx, ty)?
                }
                ValueDef::Placeholder(ty) => {
                    writeln!(f, "{}    {} = placeholder # {}", self.indent, value, ty)?
                }
                ValueDef::None => writeln!(f, "{}    {} = none", self.indent, value)?,
                _ => {}
            }
        }

        for (block_id, block) in self.body.blocks.entries() {
            let block_params = block
                .params
                .iter()
                .map(|(ty, val)| format!("{}: {}", val, ty))
                .collect::<Vec<_>>();
            writeln!(
                f,
                "{}  {}({}): # {}",
                self.indent,
                block_id,
                block_params.join(", "),
                block.desc
            )?;

            if let Some(decorator) = self.decorator {
                decorator.before_block(block_id, f)?
            };

            writeln!(
                f,
                "{}    # preds: {}",
                self.indent,
                block
                    .preds
                    .iter()
                    .map(|pred| format!("{} ({})", pred, self.body.blocks[*pred].desc))
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
            writeln!(
                f,
                "{}    # succs: {}",
                self.indent,
                block
                    .succs
                    .iter()
                    .map(|succ| format!("{} ({})", succ, self.body.blocks[*succ].desc))
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
            for (_, param) in &block.params {
                if let Some(local) = self.body.value_locals[*param] {
                    writeln!(f, "{}    # {}: {}", self.indent, param, local)?;
                }
            }
            for &inst in &block.insts {
                if let Some(local) = self.body.value_locals[inst] {
                    writeln!(f, "{}    # {}: {}", self.indent, inst, local)?;
                }
                match &self.body.values[inst] {
                    ValueDef::Operator(op, args, tys) => {
                        let args = self.body.arg_pool[*args]
                            .iter()
                            .map(|&v| format!("{}", v))
                            .collect::<Vec<_>>();
                        let tys = self.body.type_pool[*tys]
                            .iter()
                            .map(|&ty| format!("{}", ty))
                            .collect::<Vec<_>>();
                        let loc = if self.body.source_locs[inst] != SourceLoc::invalid()
                            && self.module.is_some()
                        {
                            let module = self.module.as_ref().unwrap();
                            let loc = self.body.source_locs[inst];
                            let data = &module.debug.source_locs[loc];
                            let filename = &module.debug.source_files[data.file];
                            format!("@{} {}:{}:{}", loc, filename, data.line, data.col)
                        } else {
                            "".to_owned()
                        };
                        write!(
                            f,
                            "{}    {} = {} {} # {} {} ",
                            self.indent,
                            inst,
                            op,
                            args.join(", "),
                            tys.join(", "),
                            loc,
                        )?;
                        if let Some(decorator) = self.decorator {
                            decorator.after_inst(inst, f)?;
                        }
                        writeln!(f, "")?;
                    }
                    ValueDef::PickOutput(val, idx, ty) => {
                        writeln!(f, "{}    {} = {}.{} # {}", self.indent, inst, val, idx, ty)?;
                    }
                    ValueDef::Alias(val) => {
                        writeln!(f, "{}    {} = {}", self.indent, inst, val)?;
                    }
                    _ => unreachable!(),
                }
            }
            if let Some(decorator) = self.decorator {
                decorator.after_block(block_id, f)?;
            }
            writeln!(f, "{}    {}", self.indent, block.terminator)?;
        }

        if let Some(decorator) = self.decorator {
            decorator.after_function_body(f)?;
        }
        writeln!(f, "{}}}", self.indent)?;

        Ok(())
    }
}

pub struct ModuleDisplay<'a, PD: PrintDecorator> {
    pub(crate) module: &'a Module<'a>,
    pub(crate) decorators: Option<HashMap<super::Func, &'a PD>>,
}

impl<'a, PD: PrintDecorator> Display for ModuleDisplay<'a, PD> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        writeln!(f, "module {{")?;
        if let Some(func) = self.module.start_func {
            writeln!(f, "    start = {}", func)?;
        }
        let mut sig_strs = HashMap::new();
        for (sig, sig_data) in self.module.signatures.entries() {
            let arg_tys = sig_data
                .params
                .iter()
                .map(|&ty| format!("{}", ty))
                .collect::<Vec<_>>();
            let ret_tys = sig_data
                .returns
                .iter()
                .map(|&ty| format!("{}", ty))
                .collect::<Vec<_>>();
            let sig_str = format!("{} -> {}", arg_tys.join(", "), ret_tys.join(", "));
            sig_strs.insert(sig, sig_str.clone());
            writeln!(f, "  {}: {}", sig, sig_str)?;
        }
        for (global, global_data) in self.module.globals.entries() {
            writeln!(
                f,
                "  {}: {:?} # {}",
                global, global_data.value, global_data.ty
            )?;
        }
        for (table, table_data) in self.module.tables.entries() {
            writeln!(f, "  {}: {}", table, table_data.ty)?;
            if let Some(funcs) = &table_data.func_elements {
                for (i, &func) in funcs.iter().enumerate() {
                    writeln!(f, "    {}[{}]: {}", table, i, func)?;
                }
            }
        }
        for (memory, memory_data) in self.module.memories.entries() {
            writeln!(
                f,
                "  {}: initial {} max {:?}",
                memory, memory_data.initial_pages, memory_data.maximum_pages
            )?;
            for seg in &memory_data.segments {
                writeln!(
                    f,
                    "    {} offset {}: # {} bytes",
                    memory,
                    seg.offset,
                    seg.data.len()
                )?;
            }
        }
        for import in &self.module.imports {
            writeln!(
                f,
                "  import \"{}\".\"{}\": {}",
                import.module, import.name, import.kind
            )?;
        }
        for export in &self.module.exports {
            writeln!(f, "  export \"{}\": {}", export.name, export.kind)?;
        }
        for (func, func_decl) in self.module.funcs.entries() {
            match func_decl {
                FuncDecl::Body(sig, name, body) => {
                    writeln!(
                        f,
                        "  {} \"{}\": {} = # {}",
                        func,
                        name,
                        sig,
                        sig_strs.get(&sig).unwrap()
                    )?;
                    writeln!(f, "{}", body.display("    ", Some(self.module)))?;
                }
                FuncDecl::Lazy(sig, name, reader) => {
                    writeln!(
                        f,
                        "  {} \"{}\": {} = # {}",
                        func,
                        name,
                        sig,
                        sig_strs.get(&sig).unwrap()
                    )?;
                    writeln!(f, "  # raw bytes (length {})", reader.range().len())?;
                }
                FuncDecl::Compiled(sig, name, _) => {
                    writeln!(
                        f,
                        "  {} \"{}\": {} = # {}",
                        func,
                        name,
                        sig,
                        sig_strs.get(&sig).unwrap()
                    )?;
                    writeln!(f, "  # already compiled")?;
                }
                FuncDecl::Import(sig, name) => {
                    writeln!(
                        f,
                        "  {} \"{}\": {} # {}",
                        func,
                        name,
                        sig,
                        sig_strs.get(&sig).unwrap()
                    )?;
                }
                FuncDecl::None => {
                    writeln!(f, "  {}: none", func)?;
                }
            }
        }
        for (loc, loc_data) in self.module.debug.source_locs.entries() {
            writeln!(
                f,
                "  {} = {} line {} column {}",
                loc, loc_data.file, loc_data.line, loc_data.col
            )?;
        }
        for (file, file_name) in self.module.debug.source_files.entries() {
            writeln!(f, "  {} = \"{}\"", file, file_name)?;
        }
        writeln!(f, "}}")?;
        Ok(())
    }
}
