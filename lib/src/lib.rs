extern crate futures;

use futures::{Stream, Future, Poll};

///! a future holding something else until it is dropped
pub struct MarkedFuture<I,E,O,F>
    where F : Future<Item=I, Error=E>
{
    f: F,
    o: O,
}

pub fn mark_future<I,E,O,F>(f: F, o: O)
    -> MarkedFuture<I,E,O,F>
    where F : Future<Item=I, Error=E>
{
    MarkedFuture{
        f,
        o,
    }
}

impl<I,E,O,F> Future for MarkedFuture<I,E,O,F>
    where F : Future<Item=I, Error=E>
{
    type Item  = I;
    type Error = E;
    fn poll(&mut self) -> Poll<I,E> {
        let _ = &self.o;
        self.f.poll()
    }
}

///! a stream holding something else until it is dropped
pub struct MarkedStream<I,E,O,F>
    where F : Stream<Item=I, Error=E>
{
    f: F,
    o: O,
}

pub fn mark_stream<I,E,O,F>(f: F, o: O)
    -> MarkedStream<I,E,O,F>
    where F : Stream<Item=I, Error=E>
{
    MarkedStream{
        f,
        o,
    }
}

impl<I,E,O,F> Stream for MarkedStream<I,E,O,F>
    where F : Stream<Item=I, Error=E>
{
    type Item  = I;
    type Error = E;
    fn poll(&mut self) -> Poll<Option<I>,E> {
        let _ = &self.o;
        self.f.poll()
    }
}

#[test]
pub fn bla() {
    let blurp = futures::future::ok(1) as futures::future::FutureResult<_,()>;
    let mut blurp = mark_future(blurp, 3);
    assert_eq!(blurp.poll(), Ok(futures::Async::Ready(1)));
}
