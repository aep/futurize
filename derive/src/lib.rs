#![recursion_limit="512"]

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
                    let meta = attr.interpret_meta().expect("cannot parse as meta");
                    let meta = match meta {
                        syn::Meta::NameValue(m) => m,
                        _ =>  panic!("needs name value pair like '#[returns = \"u8\"]"),
                    };

                    let meta = match meta.lit {
                        syn::Lit::Str(s) => s.value(),
                        _ =>  panic!("needs name value pair like '#[returns = \"u8\"]"),
                    };

                    let meta : syn::Type = syn::parse_str(&meta).unwrap();
                    returns = meta.into_token_stream();
                }
            }
        }


        let varname_    = varname.clone();
        rets.push(quote!{
            #varname_(#returns)
        });


        let args_ = args.clone();
        trait_fns.push(quote! {
            fn #fname(self, #(#args_),*) -> R<Self,#returns>;
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
                    .and_then(|r|r)
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
        let then = quote!{
            then(|v|{
                match v {
                    Err((s,e)) => {
                        ret.send(Err(e)).ok();
                        s
                    },
                    Ok((s,v)) => {
                        ret.send(Ok(Return::#varname_(v))).ok();
                        s
                    }
                }.ok_or(())
            })
        };
        let mcall = if argnames.len() > 0 { quote! {
            #name::#varname { #(#argnames),* } => {
                let ft = t.#fname(#(#argnames_),*)
                    .#then;
                Box::new(ft) as Box<Future<Item=T,Error=()> + Send + Sync>
            }
        }} else { quote! {
            #name::#varname => {
                let ft = t.#fname()
                    .#then;
                Box::new(ft) as Box<Future<Item=T,Error=()> + Send + Sync>
            }
        }};

        matches.push(mcall);
    }

    let matches_ = matches.clone();
    let expanded = quote! {
        use futures;
        use futures::Stream;
        use futures::Sink;
        use futures::Future;
        use futures::sync::oneshot;
        use failure::Error;
        use std::time::Instant;
        use futures::future::Either;

        #[derive(Clone)]
        pub struct Handle {
            tx: futures::sync::mpsc::Sender<(oneshot::Sender<Result<Return,Error>>, #name)>,
        }
        impl Handle {
            #(#call_fns)*
        }

        enum Return {
            #(#rets),*
        }

        pub type R<S: Worker + Sized, T> = Box<Future<Item=(Option<S>, T), Error=(Option<S>, Error)> + Sync + Send>;

        pub trait Worker
            where Self: Sized,
        {

            #(#trait_fns)*

            fn canceled(self) {}
            fn interval(self, Instant) -> Box<Future<Item=Option<Self>, Error=()> + Sync + Send> {
                panic!("must implement Worker::interval if using spawn_with_interval");
            }
        }

        pub fn spawn<T: Worker> (buffer: usize, t: T)
            -> (impl Future<Item=(), Error=()>, Handle)
            where T: 'static + Send + Sync
        {
            let (tx,rx) = futures::sync::mpsc::channel(buffer);

            let ft = rx.fold(t, |t, (ret, m) : (oneshot::Sender<Result<Return, Error>>, #name)|{
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

        pub fn spawn_with_interval<T: Worker, I: Stream<Item=Instant, Error=()>> (buffer: usize, t: T, i : I)
            -> (impl Future<Item=(), Error=()>, Handle)
            where T: 'static + Send + Sync
        {
            let (tx,rx) = futures::sync::mpsc::channel(buffer);

            let i  = i.map(|i|Either::A(i));
            let rx = rx.map(|i|Either::B(i));
            let rx = rx.select(i);

            let ft = rx.fold(t, |t, either : Either<Instant, (oneshot::Sender<Result<Return, Error>>, #name)>|{
                match either {
                    Either::A(i) => {
                        let ft = t.interval(i)
                            .and_then(|v|{
                                v.ok_or(())
                            });
                        Box::new(ft) as Box<Future<Item=T,Error=()> + Send + Sync>
                    },
                    Either::B((ret,m)) => match m {
                        #(#matches_),*
                    }
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
