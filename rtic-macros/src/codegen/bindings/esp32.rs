use crate::{
    analyze::Analysis as CodegenAnalysis,
    syntax::{analyze::Analysis as SyntaxAnalysis, ast::App},
    codegen::util,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse, Attribute, Ident};


//#[cfg((feature = esp32c3)]
#[allow(clippy::too_many_arguments)]
pub fn impl_mutex(
    app: &App,
    _analysis: &CodegenAnalysis,
    cfgs: &[Attribute],
    resources_prefix: bool,
    name: &Ident,
    ty: &TokenStream2,
    ceiling: u8,
    ptr: &TokenStream2,
) -> TokenStream2 {
    let path = if resources_prefix {
        quote!(shared_resources::#name)
    } else {
        quote!(#name)
    };

    let device = &app.args.device;
    quote!(
        #(#cfgs)*
        impl<'a> rtic::Mutex for #path<'a> {
            type T = #ty;

            #[inline(always)]
            fn lock<RTIC_INTERNAL_R>(&mut self, f: impl FnOnce(&mut #ty) -> RTIC_INTERNAL_R) -> RTIC_INTERNAL_R {
                /// Priority ceiling
                const CEILING: u8 = #ceiling;

                unsafe {
                    rtic::export::lock(
                        #ptr,
                        CEILING,
                        #device::NVIC_PRIO_BITS,
                        f,
                    )
                }
            }
        }
    )
}

pub fn extra_assertions(_: &App, _: &SyntaxAnalysis) -> Vec<TokenStream2> {
    vec![]
}


pub fn pre_init_checks(app: &App, _: &SyntaxAnalysis) -> Vec<TokenStream2> {
    let mut stmts = vec![];

    // check that all dispatchers exists in the `Interrupt` enumeration regardless of whether
    // they are used or not
    let interrupt = util::interrupt_ident();
    let rt_err = util::rt_err_ident();

    for name in app.args.dispatchers.keys() {
        stmts.push(quote!(let _ = #rt_err::#interrupt::#name;));
    }
    stmts
}
pub fn pre_init_enable_interrupts(app: &App, analysis: &CodegenAnalysis) -> Vec<TokenStream2> {
    let mut stmts = vec![];

    let interrupt = util::interrupt_ident();
    let rt_err = util::rt_err_ident();
    let device = &app.args.device;
    let max_prio:usize = 15; //unfortunately this is not part of pac, but we know that max prio is 15.
    let interrupt_ids = analysis.interrupts.iter().map(|(p, (id, _))| (p, id));
    // Unmask interrupts and set their priorities
    for (&priority, name) in interrupt_ids.chain(app.hardware_tasks.values().filter_map(|task| {
        Some((&task.args.priority, &task.args.binds))
    })) {
        let es = format!(
            "Maximum priority used by interrupt vector '{name}' is more than supported by hardware"
        );
        // Compile time assert that this priority is supported by the device
        stmts.push(quote!(
            const _: () =  if (#max_prio) <= #priority as usize { ::core::panic!(#es); };
        ));
        // NOTE unmask the interrupt *after* setting its priority: changing the priority of a pended
        // interrupt is implementation defined
        stmts.push(quote!(
            rtic::export::hal_interrupt::enable(
                #rt_err::Interrupt::#name,
                rtic::export::int_to_prio(#priority)            
            );
        ));
    }
    stmts
}


pub fn architecture_specific_analysis(_app: &App, _analysis: &SyntaxAnalysis) -> parse::Result<()> {
    Ok(())
}

pub fn interrupt_entry(_app: &App, _analysis: &CodegenAnalysis) -> Vec<TokenStream2> {
    let mut stmts = vec![];

    //we may enable interrupts globally on entry eventually, but first priority threshold must be implemented in assembly.
    //stmts.push(quote!(riscv::interrupt::enable));

    stmts
}

pub fn interrupt_exit(_app: &App, _analysis: &CodegenAnalysis) -> Vec<TokenStream2> {
    vec![]
}

/*pub fn int_to_prio(int:u8) -> Priority{
    match(int){
        0 => None,
        1 => Priority1,
        2 => Priority2,
        3 => Priority3,
        4 => Priority4,
        5 => Priority5,
        6 => Priority6,
        7 => Priority7,
        8 => Priority8,
        9 => Priority9,
        10 => Priority10,
        11 => Priority11,
        12 => Priority12,
        13 => Priority13,
        14 => Priority14,
        15 => Priority15,
        _ => panic!(),
    }
}*/
