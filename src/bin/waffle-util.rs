//! WAFFLE command-line tool.

use anyhow::Result;
use log::debug;
use std::{collections::HashMap, path::PathBuf};
use structopt::StructOpt;
use waffle::{entity::EntityRef, FrontendOptions, Func, Module, NOPPrintDecorator, OptOptions};

#[derive(Debug, StructOpt)]
#[structopt(name = "waffle-util", about = "WAFFLE utility.")]
struct Options {
    #[structopt(short, long)]
    debug: bool,

    #[structopt(
        help = "Do basic optimizations: GVN and const-prop",
        long = "basic-opts"
    )]
    basic_opts: bool,

    #[structopt(
        help = "Enable parsing of debug-info from input",
        short = "g",
        long = "debug-info"
    )]
    debug_info: bool,

    #[structopt(help = "Transform to maximal SSA", long = "max-ssa")]
    max_ssa: bool,

    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "print-ir", about = "Parse Wasm and print resulting IR")]
    PrintIR {
        #[structopt(help = "Wasm file to parse")]
        wasm: PathBuf,
    },
    #[structopt(name = "print-func", about = "Parse Wasm and print one function body")]
    PrintFunc {
        #[structopt(help = "Wasm file to parse")]
        wasm: PathBuf,
        #[structopt(help = "Index of Wasm function to print")]
        func: usize,
    },
    #[structopt(name = "roundtrip", about = "Round-trip Wasm through IR")]
    RoundTrip {
        #[structopt(help = "Wasm file to parse", short = "i")]
        input: PathBuf,
        #[structopt(help = "Wasm file to produce", short = "o")]
        output: PathBuf,
    },
}

fn apply_options(opts: &Options, module: &mut Module) -> Result<()> {
    module.expand_all_funcs()?;
    if opts.basic_opts {
        module.per_func_body(|body| body.optimize(&OptOptions::default()));
    }
    if opts.max_ssa {
        module.per_func_body(|body| body.convert_to_max_ssa(None));
    }
    Ok(())
}

fn main() -> Result<()> {
    let opts = Options::from_args();

    let mut logger = env_logger::Builder::from_default_env();
    if opts.debug {
        logger.filter_level(log::LevelFilter::Debug);
    }
    let _ = logger.try_init();

    let mut options = FrontendOptions::default();
    options.debug = opts.debug_info;

    match &opts.command {
        Command::PrintIR { wasm } => {
            let bytes = std::fs::read(wasm)?;
            debug!("Loaded {} bytes of Wasm data", bytes.len());
            let mut module = Module::from_wasm_bytes(&bytes[..], &options)?;
            apply_options(&opts, &mut module)?;
            let mut nop_decorators = HashMap::new();
            let nop_decorator = NOPPrintDecorator::default();
            module.funcs.entries().into_iter().for_each(|(func, _)| {
                nop_decorators.insert(func, &nop_decorator);
            });
            println!("{}", module.display(nop_decorators));
        }
        Command::PrintFunc { wasm, func } => {
            let bytes = std::fs::read(wasm)?;
            debug!("Loaded {} bytes of Wasm data", bytes.len());
            let mut module = Module::from_wasm_bytes(&bytes[..], &options)?;
            apply_options(&opts, &mut module)?;
            println!(
                "{}",
                module.funcs[Func::new(*func)]
                    .body()
                    .unwrap()
                    .display_verbose("", Some(&module), &waffle::NOPPrintDecorator::default())
            );
        }
        Command::RoundTrip { input, output } => {
            let bytes = std::fs::read(input)?;
            debug!("Loaded {} bytes of Wasm data", bytes.len());
            let mut module = Module::from_wasm_bytes(&bytes[..], &options)?;
            apply_options(&opts, &mut module)?;
            let produced = module.to_wasm_bytes()?;
            std::fs::write(output, &produced[..])?;
        }
    }

    Ok(())
}
