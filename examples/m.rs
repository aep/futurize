#[macro_use]
extern crate futurize;
extern crate futures;
extern crate tokio;

use futures::Future;



pub mod stuff {
    #[derive(Worker)]
    pub enum Command{
        Hello{d: u32},
        Stop,
    }
}

#[derive(Default)]
struct Worker {
    counter: u32,
}

impl stuff::Worker for Worker {
    fn hello(mut self, d: u32) -> Box<Future<Item=Option<Self>, Error=()> + Sync + Send> {
        self.counter += d;
        println!("lets get work done: {}", self.counter);
        Box::new(futures::future::ok(Some(self)))
    }

    fn stop(self) -> Box<Future<Item=Option<Self>, Error=()> + Sync + Send> {
        Box::new(futures::future::ok(None))
    }

    fn canceled(self) {
        println!("canceled");
    }
}

pub fn main() {
    tokio::run(futures::lazy(||{
        let (worker, mut handle) = stuff::spawn(100, Worker::default());
        tokio::spawn(worker);
        tokio::spawn(handle.hello(4).map_err(|e|panic!(e)));
        tokio::spawn(handle.hello(5).map_err(|e|panic!(e)));
        tokio::spawn(handle.stop().map_err(|e|panic!(e)));
        tokio::spawn(handle.hello(2).map_err(|e|panic!(e)));
        Ok(())
    }));
}
