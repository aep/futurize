#[macro_use]
extern crate futurize;
extern crate futures;
extern crate failure;
extern crate tokio;

use futures::Future;

#[derive(Worker)]
pub enum Stuff{
    Hello{d: u32},
}

#[derive(Default)]
struct Worker {
    counter: u32,
}

impl stuff::Worker for Worker {
    fn hello(&mut self, d: u32) {
        self.counter += d;
        println!("lets get work done: {}", self.counter);
    }
}

pub fn main() {
    tokio::run(futures::lazy(||{
        let (worker, mut handle) = stuff::spawn(100, Worker::default());
        tokio::spawn(worker);
        tokio::spawn(handle.hello(4).map_err(|e|panic!(e)));
        tokio::spawn(handle.hello(5).map_err(|e|panic!(e)));
        Ok(())
    }));
}
