#[macro_use]
extern crate futurize_derive;
extern crate futurize;
extern crate futures;
extern crate tokio;
extern crate failure;

use futures::Future;


pub mod stuff {
    #[derive(Worker)]
    pub enum Command{
        #[returns = "(u8, u32)"]
        Hello{d: u32},
        #[returns = "std::vec::Vec<u8>"]
        Stop,
    }
}

#[derive(Default)]
struct Worker {
    counter: u32,
}

impl stuff::Worker for Worker {
    fn hello(mut self, d: u32) -> Box<Future<Item=(Option<Self>, (u8,u32)), Error=()> + Sync + Send> {
        self.counter += d;
        println!("lets get work done: {}", self.counter);
        Box::new(futures::future::ok((Some(self), (8,32))))
    }

    fn stop(self) -> Box<Future<Item=(Option<Self>, Vec<u8>), Error=()> + Sync + Send> {
        Box::new(futures::future::ok((None, vec![8,2])))
    }

    fn canceled(self) {
        println!("canceled");
    }
}

pub fn main() {
    tokio::run(futures::lazy(||{
        let (worker, mut handle) = stuff::spawn(100, Worker::default());
        tokio::spawn(worker);
        handle.hello(4)
        .and_then(move |r|{
            println!("{:?}", r);
            handle.stop()
        })
        .and_then(|r|{
            println!("{:?}", r);
            Ok(())
        })
        .map_err(|e|panic!(e))


        //tokio::spawn(handle.hello(4).map_err(|e|panic!(e)));
        //tokio::spawn(handle.hello(5).map_err(|e|panic!(e)));
        //tokio::spawn(handle.stop().map_err(|e|panic!(e)));
        //tokio::spawn(handle.hello(2).map_err(|e|panic!(e)));
    }));
}
