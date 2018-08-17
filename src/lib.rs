#![recursion_limit="256"]

extern crate proc_macro;
extern crate proc_macro2;
extern crate futures;
extern crate syn;
extern crate failure;
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
    let mod_name        = Ident::new(&format!("{}", name).to_snake_case(), name.span());

    let mut call_fns    = Vec::new();
    let mut trait_fns   = Vec::new();
    let mut matches     = Vec::new();

    for variant in dnum.variants {
        let mut args = Vec::new();
        let mut argnames  = Vec::new();
        args.push(quote!{&mut self});
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
            fn #fname(#(#args_),*);
        });

        let name_       = name.clone();
        let varname_    = varname.clone();
        let argnames_   = argnames.clone();
        call_fns.push(quote! {
            pub fn #fname(#(#args),*) -> impl futures::Future<Item=(), Error=futures::sync::mpsc::SendError<#name>> {
                self.tx.clone().send(#name_::#varname_{#(#argnames_),*}).and_then(|_|Ok(()))
            }
        });

        let argnames_ = argnames.clone();
        matches.push(quote! {
            #name::#varname { #(#argnames),* } => self.t.#fname(#(#argnames_),*),
        });
    }

    let expanded = quote! {mod #mod_name {
        use super::#name;
        use futures;
        use futures::Stream;
        use futures::Sink;
        use futures::Future;

        pub struct Job<T: Worker> {
            t:  T,
            rx: futures::sync::mpsc::Receiver<#name>,
        }

        #[derive(Clone)]
        pub struct Handle {
            tx: futures::sync::mpsc::Sender<#name>,
        }
        impl Handle {
            #(#call_fns)*
        }

        pub trait Worker {
            #(#trait_fns)*
        }

        pub fn spawn<T: Worker> (buffer: usize, t: T) -> (Job<T>, Handle) {
            let (tx,rx) = futures::sync::mpsc::channel(buffer);

            (
                Job {
                    t,
                    rx,
                },
                Handle {
                    tx,
                },
                )
        }

        // The generated impl.
        impl<T: Worker> futures::Future for Job<T> {
            type Item  = ();
            type Error = ();

            fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
                loop {
                    match self.rx.poll()? {
                        futures::Async::NotReady     => return Ok(futures::Async::NotReady),
                        futures::Async::Ready(None)  => return Ok(futures::Async::Ready(())),
                        futures::Async::Ready(Some(m)) => match m {
                            #(#matches),*
                        }
                    }
                }
            }
        }
    }};

    // Hand the output tokens back to the compiler.
    expanded.into()
}
