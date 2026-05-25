pub mod ast;
mod cli;
mod common;
mod schema;
mod util;

use std::{fs, path::Path};

use anyhow::Context;
use proc_macro2::TokenStream;
use quote::quote;

pub fn generate(json: &str, out_dir: &Path) -> anyhow::Result<()> {
    let fml: ast::Fml = serde_json::from_str(json).context("parsing FML JSON")?;

    if out_dir.exists() {
        std::fs::remove_dir_all(out_dir)
            .with_context(|| format!("clearing {}", out_dir.display()))?;
    }
    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let options = common::options::generate(&fml.options).context("generating option types")?;
    write(out_dir, "options", options)?;

    let cameras = common::cameras::generate(&fml.cameras).context("generating camera registry")?;
    write(out_dir, "cameras", cameras)?;

    let simulations = common::simulations::generate(&fml.options, &fml.cameras)
        .context("generating simulation types")?;
    write(out_dir, "simulations", simulations)?;

    let renders = common::renders::generate(&fml.options, &fml.cameras)
        .context("generating render profile types")?;
    write(out_dir, "renders", renders)?;

    let cli = cli::generate(&fml.options, &fml.cameras).context("generating CLI args")?;
    write(out_dir, "cli", cli)?;

    let mod_rs = root(&fml);
    write(out_dir, "mod", mod_rs)?;

    Ok(())
}

fn root(fml: &ast::Fml) -> TokenStream {
    let banner = format!(
        "Generated via codegen. Do not edit. \
         Inventory: {} cameras, {} options",
        fml.cameras.len(),
        fml.options.len(),
    );

    quote! {
        #![doc = #banner]

        pub mod cameras;
        pub mod options;
        pub mod simulations;
        pub mod renders;
        pub mod cli;
    }
}

fn write(out_dir: &Path, name: &str, tokens: TokenStream) -> anyhow::Result<()> {
    let formatted =
        format(tokens).with_context(|| format!("formatting generated module `{name}`"))?;
    let path = out_dir.join(format!("{name}.rs"));
    fs::write(&path, formatted).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn format(tokens: TokenStream) -> anyhow::Result<String> {
    let file: syn::File = syn::parse2(tokens).context("parsing generated TokenStream")?;
    Ok(prettyplease::unparse(&file))
}
