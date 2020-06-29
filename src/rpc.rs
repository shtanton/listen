use std::hash;

use iced::futures::{future, io, prelude::*, stream::BoxStream};

use iced::Subscription;

#[derive(Debug, Clone)]
pub enum Receive {
    Time,
}

pub struct Rpc;

impl Rpc {
    pub fn new() -> Self {
        Self
    }
    pub fn receive(&self) -> Subscription<Receive> {
        Subscription::from_recipe(RpcSubscription)
    }
    pub fn send<T: ToString>(&mut self, data: T) {
        println!("{}", data.to_string());
    }
}

struct RpcSubscription;

impl<H, I> iced_native::subscription::Recipe<H, I> for RpcSubscription
where
    H: hash::Hasher,
{
    type Output = Receive;
    fn hash(&self, state: &mut H) {
        use hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
    }
    fn stream(self: Box<Self>, _input: BoxStream<'static, I>) -> BoxStream<'static, Self::Output> {
        let stdin = io::BufReader::new(io::AllowStdIo::new(std::io::stdin()));
        Box::pin(stdin.lines().filter_map(|line| {
            future::ready(
                line.ok()
                    .map(|line| match line.as_str() {
                        "time" => Some(Receive::Time),
                        _ => None,
                    })
                    .flatten(),
            )
        }))
    }
}
