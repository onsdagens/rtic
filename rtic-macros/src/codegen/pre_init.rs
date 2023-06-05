use super::bindings::{pre_init_checks, pre_init_enable_interrupts};
use crate::analyze::Analysis;
use crate::syntax::ast::App;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use crate::codegen::bindings::{peripheral_access, get_perip};

/// Generates code that runs before `#[init]`
pub fn codegen(app: &App, analysis: &Analysis) -> Vec<TokenStream2> {
    let mut stmts = vec![];
    let perips = peripheral_access();
    let perip_get = get_perip();

    // Disable interrupts -- `init` must run with interrupts disabled
    stmts.push(quote!(rtic::export::interrupt::disable();));

    stmts.push(quote!(
        // To set the variable in cortex_m so the peripherals cannot be taken multiple times
        let mut core: rtic::export::#perips = rtic::export::#perips::#perip_get.into();
    ));

    stmts.append(&mut pre_init_checks(app, analysis));

    stmts.append(&mut pre_init_enable_interrupts(app, analysis));

    stmts
}
