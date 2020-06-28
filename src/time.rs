use iced::futures::{self, StreamExt};
use std::time::Instant;

pub fn every(
    duration: std::time::Duration,
) -> iced::Subscription<()> {
    iced::Subscription::from_recipe(Every(duration))
}

struct Every(std::time::Duration);

impl<H, I> iced_native::subscription::Recipe<H, I> for Every
where
    H: std::hash::Hasher,
{
    type Output = ();

    fn hash(&self, state: &mut H) {
        use std::hash::Hash;

        std::any::TypeId::of::<Self>().hash(state);
        self.0.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: futures::stream::BoxStream<'static, I>,
    ) -> futures::stream::BoxStream<'static, Self::Output> {
        let duration = self.0;
        futures::stream::unfold(Instant::now(), move |state| async move {
            let passed = Instant::now() - state;
            let remaining = duration - passed;
            std::thread::sleep(remaining);
            Some(((), state + duration))
        })
            .boxed()
    }
}
