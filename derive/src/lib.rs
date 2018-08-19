#![recursion_limit="256"]

extern crate proc_macro;
extern crate proc_macro2;
extern crate syn;
extern crate heck;

#[macro_use]
extern crate quote;

use proc_macro::TokenStream;
use proc_macro2::{Ident};
use syn::DeriveInput;
use heck::SnakeCase;
use quote::ToTokens;


#[proc_macro_derive(Worker, attributes(returns))]
pub fn derive_worker(input: TokenStream) -> TokenStream {

    let ast: DeriveInput = syn::parse(input).unwrap();
    let dnum = match ast.data {
        syn::Data::Enum(v) => v,
        _ => panic!("must be enum"),
    };

    let name            = &ast.ident;

    let mut rets        = Vec::new();
    let mut call_fns    = Vec::new();
    let mut trait_fns   = Vec::new();
    let mut matches     = Vec::new();

    for variant in dnum.variants {
        let varname = variant.ident;
        let fname = Ident::new(&format!("{}", varname).to_snake_case(), varname.span());
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

        let mut returns = quote!(());
        for attr in variant.attrs {
            if attr.path.segments.len() == 1 {
                if format!("{}", attr.path.segments[0].ident) == "returns" {
                    let meta = attr.interpret_meta().unwrap();
                    let meta = match meta {
                        syn::Meta::List(m) => m,
                        _ =>  panic!("expected syntax like '#[returns(u8)]"),
                    };
                    if meta.nested.len() == 1 {
                        let meta = match meta.nested[0] {
                            syn::NestedMeta::Meta(ref m) => m,
                            _ => panic!("expected syntax like '#[returns(u8)]"),
                        };
                        returns = meta.into_token_stream();
                    } else {
                        let mut rs = Vec::new();
                        for meta in meta.nested {
                            let meta = match meta {
                                syn::NestedMeta::Meta(ref m) => m,
                                _ => panic!("expected syntax like '#[returns(u8)]"),
                            };
                            rs.push(meta.into_token_stream());
                        };
                        returns = quote!{(#(#rs),*)};
                    }
                }
            }
        }


        let varname_    = varname.clone();
        rets.push(quote!{
            #varname_(#returns)
        });


        let args_ = args.clone();
        trait_fns.push(quote! {
            fn #fname(self, #(#args_),*) -> Box<Future<Item=(Option<Self>, #returns),Error=()> + Sync + Send>;
        });

        let name_       = name.clone();
        let varname_    = varname.clone();
        let argnames_   = argnames.clone();
        let callarg = if argnames.len() > 0 {quote! {
            #name_::#varname_{#(#argnames_),*}
        }} else { quote! {
            #name_::#varname_
        }};
        call_fns.push(quote!{
            pub fn #fname(&mut self, #(#args),*) -> impl futures::Future<Item=#returns, Error=Error> {
                let (tx,rx) = oneshot::channel();
                self.tx.clone().send((tx, #callarg))
                    .map_err(Error::from)
                    .and_then(|_|rx.map_err(Error::from))
                    .map(|v|{
                        match v {
                            Return::#varname_(r) => r,
                            _ => unreachable!()
                        }
                    })
                    .map_err(Error::from)
            }
        });

        let argnames_ = argnames.clone();
        let mcall = if argnames.len() > 0 { quote! {
            #name::#varname { #(#argnames),* } => {
                let ft = t.#fname(#(#argnames_),*)
                    .and_then(|v|{
                        ret.send(Return::#varname_(v.1)).ok();
                        v.0.ok_or(())
                    });
                Box::new(ft) as Box<Future<Item=T,Error=()> + Send + Sync>
            }
        }} else { quote! {
            #name::#varname => {
                let ft = t.#fname()
                    .and_then(|v|{
                        ret.send(Return::#varname_(v.1)).ok();
                        v.0.ok_or(())
                    });
                Box::new(ft) as Box<Future<Item=T,Error=()> + Send + Sync>
            }
        }};

        matches.push(mcall);
    }

    let expanded = quote! {
        use futures;
        use futures::Stream;
        use futures::Sink;
        use futures::Future;
        use futures::sync::oneshot;
        use failure::Error;

        #[derive(Clone)]
        pub struct Handle {
            tx: futures::sync::mpsc::Sender<(oneshot::Sender<Return>, #name)>,
        }
        impl Handle {
            #(#call_fns)*
        }

        enum Return {
            #(#rets),*
        }

        pub trait Worker
            where Self: Sized,
        {
            #(#trait_fns)*

            fn canceled(self) {}
        }

        pub fn spawn<T: Worker> (buffer: usize, t: T)
            -> (impl Future<Item=(), Error=()>, Handle)
            where T: 'static + Send + Sync
        {
            let (tx,rx) = futures::sync::mpsc::channel(buffer);


            let ft = rx.fold(t, |t, (ret, m) : (oneshot::Sender<Return>, #name)|{
                match m {
                    #(#matches),*
                }
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
