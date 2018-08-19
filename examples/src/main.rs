#[macro_use]
extern crate futurize_derive;
extern crate futurize;
extern crate futures;
extern crate tokio;
extern crate failure;

use futures::Future;
use std::time::{Duration, Instant};
use futures::Stream;


pub mod stuff {
    #[derive(Worker)]
    pub enum Command{
        #[returns = "(u8, u32)"]
        Hello{d: u32},
        #[returns = "Vec<u8>"]
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

    fn interval(self, _now : std::time::Instant) -> Box<Future<Item=Option<Self>, Error=()> + Sync + Send> {
        println!("interval");
        Box::new(futures::future::ok(Some(self)))
    }

    fn canceled(self) {
        println!("canceled");
    }
}

pub fn main() {
    tokio::run(futures::lazy(||{
        let gc = tokio::timer::Interval::new(Instant::now(), Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(2));
        let gc = gc.map_err(|e|panic!(e));
        let (worker, mut handle) = stuff::spawn_with_interval(100, Worker::default(), gc);
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
