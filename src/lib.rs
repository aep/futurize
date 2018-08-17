#![recursion_limit="256"]

extern crate proc_macro;
extern crate proc_macro2;
extern crate futures;
extern crate syn;
extern crate heck;

#[macro_use]
extern crate quote;

use proc_macro::TokenStream;
use proc_macro2::{Ident};
use syn::DeriveInput;
use heck::SnakeCase;
use quote::ToTokens;


#[proc_macro_derive(Worker)]
pub fn derive_worker(input: TokenStream) -> TokenStream {

    let ast: DeriveInput = syn::parse(input).unwrap();
    let dnum = match ast.data {
        syn::Data::Enum(v) => v,
        _ => panic!("must be enum"),
    };

    let name            = &ast.ident;

    let mut call_fns    = Vec::new();
    let mut trait_fns   = Vec::new();
    let mut matches     = Vec::new();

    for variant in dnum.variants {
        let mut args = Vec::new();
        let mut argnames  = Vec::new();
        match variant.fields {
            syn::Fields::Named(fields) => {
                for field in fields.named {
                    let name = field.ident.unwrap();
                    let typ  = field.ty.into_token_stream();

                    args.push(quote!{
                        #name : #typ
                    });
                    argnames.push(name);
                }
            },
            syn::Fields::Unnamed(_) => {
                panic!("cannot use unnamed args");
            },
            syn::Fields::Unit => (),
        };


        let varname = variant.ident;
        let fname = Ident::new(&format!("{}", varname).to_snake_case(), varname.span());

        let args_ = args.clone();
        trait_fns.push(quote! {
            fn #fname(self, #(#args_),*) -> Box<Future<Item=Option<Self>,Error=()> + Sync + Send>;
        });

        let name_       = name.clone();
        let varname_    = varname.clone();
        let argnames_   = argnames.clone();
        call_fns.push(if argnames.len() > 0 {quote! {
            pub fn #fname(&mut self, #(#args),*) -> impl futures::Future<Item=(), Error=futures::sync::mpsc::SendError<#name>> {
                self.tx.clone().send(#name_::#varname_{#(#argnames_),*}).and_then(|_|Ok(()))
            }
        }} else { quote! {
            pub fn #fname(&mut self, #(#args),*) -> impl futures::Future<Item=(), Error=futures::sync::mpsc::SendError<#name>> {
                self.tx.clone().send(#name_::#varname_).and_then(|_|Ok(()))
            }
        }});

        let argnames_ = argnames.clone();
        matches.push( if argnames.len() > 0 { quote! {
            #name::#varname { #(#argnames),* } => t.#fname(#(#argnames_),*)
        }} else { quote! {
            #name::#varname => t.#fname()
        }});
    }

    let expanded = quote! {
        use futures;
        use futures::Stream;
        use futures::Sink;
        use futures::Future;

        #[derive(Clone)]
        pub struct Handle {
            tx: futures::sync::mpsc::Sender<#name>,
        }
        impl Handle {
            #(#call_fns)*
        }

        pub trait Worker
            where Self: Sized,
        {
            #(#trait_fns)*

            fn canceled(self) {}
        }

        pub fn spawn<T: Worker> (buffer: usize, t: T) -> (impl Future<Item=(), Error=()>, Handle) {
            let (tx,rx) = futures::sync::mpsc::channel(buffer);


            let ft = rx.fold(t, |t, m|{
                match m {
                    #(#matches),*
                }.and_then(|v|v.ok_or(()))
            }).and_then(|t|{
                t.canceled();
                Ok(())
            });

            (
                ft,
                Handle {
                    tx,
                },
            )
        }
    };

    // Hand the output tokens back to the compiler.
    expanded.into()
}
