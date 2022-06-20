use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait IoFutureState<IO: Sized + Unpin>: Sized + Unpin {
    fn poll(&mut self, cx: &mut Context<'_>, io: &mut IO) -> Poll<io::Result<()>>;
    fn into_future(self, io: IO) -> IoFuture<Self, IO> {
        IoFuture::new(self, io)
    }
}

pub struct IoFuture<S: IoFutureState<IO>, IO: Sized + Unpin>(Option<(S, IO)>);

impl<S: IoFutureState<IO>, IO: Unpin> IoFuture<S, IO> {
    pub fn new(state: S, io: IO) -> Self {
        IoFuture(Some((state, io)))
    }
    pub fn checkpoint(self) -> (S, IO) {
        self.0.unwrap()
    }
}

impl<S: IoFutureState<IO>, IO: Unpin> Future for IoFuture<S, IO> {
    type Output = io::Result<IO>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (mut state, mut io) = self.0.take().unwrap();
        let p = state.poll(cx, &mut io);
        self.0 = Some((state, io));
        p.map(|r| r.map(|()| self.0.take().unwrap().1))
    }
}

pub trait IoFutureWithOutputState<IO: Sized + Unpin, O>: Sized + Unpin {
    fn poll(&mut self, cx: &mut Context<'_>, io: &mut IO) -> Poll<io::Result<O>>;
    fn into_future(self, io: IO) -> IoFutureWithOutput<Self, IO, O> {
        IoFutureWithOutput::new(self, io)
    }
}

pub struct IoFutureWithOutput<S: IoFutureWithOutputState<IO, O>, IO: Sized + Unpin, O: 'static>(
    Option<(S, IO, PhantomData<&'static O>)>,
);

impl<S: IoFutureWithOutputState<IO, O>, IO: Unpin, O> IoFutureWithOutput<S, IO, O> {
    pub fn new(state: S, io: IO) -> Self {
        IoFutureWithOutput(Some((state, io, PhantomData::default())))
    }
    pub fn checkpoint(self) -> (S, IO) {
        let (state, io, _) = self.0.unwrap();
        (state, io)
    }
}

impl<S: IoFutureWithOutputState<IO, O>, IO: Unpin, O> Future for IoFutureWithOutput<S, IO, O> {
    type Output = io::Result<(IO, O)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (mut state, mut io, _) = self.0.take().unwrap();
        let p = state.poll(cx, &mut io);
        self.0 = Some((state, io, PhantomData::default()));
        p.map(|r| r.map(|o| (self.0.take().unwrap().1, o)))
    }
}
