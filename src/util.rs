pub mod bitcoin;

use log::info;
use std::time::{Duration, Instant};

pub struct BottleCheck {
    accumulated: std::time::Duration,
    name: String,
}

impl BottleCheck {
    pub fn new(name: String) -> BottleCheck {
        Self {
            name,
            accumulated: Duration::default(),
        }
    }
    pub fn check<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let res = f();
        self.accumulated += start.elapsed();
        if self.accumulated > Duration::from_secs(30) {
            info!("Bottleneck at {}", self.name);
            self.accumulated = Duration::default();
        }
        res
    }

    pub fn check_iter<I>(&mut self, iter: I) -> BottleCheckIter<I>
    where
        I: Iterator,
    {
        BottleCheckIter { inner: self, iter }
    }
}

pub struct BottleCheckIter<'a, I> {
    inner: &'a mut BottleCheck,
    iter: I,
}

impl<'a, I> Iterator for BottleCheckIter<'a, I>
where
    I: Iterator,
{
    type Item = <I as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            ref mut inner,
            ref mut iter,
        } = self;
        inner.check(|| iter.next())
    }
}

pub fn reversed<I, II, IT>(i: I) -> I
where
    I: IntoIterator<IntoIter = II, Item = IT>,
    II: DoubleEndedIterator<Item = IT>,
    I: std::iter::FromIterator<IT>,
{
    i.into_iter().rev().collect()
}
