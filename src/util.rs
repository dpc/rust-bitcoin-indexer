pub mod bitcoin;

use log::info;
use std::time::{Duration, Instant};

pub struct BottleCheck {
    accumulated: std::time::Duration,
    last: std::time::Instant,
    name: String,
}

impl BottleCheck {
    pub fn new(name: String) -> BottleCheck {
        Self {
            name,
            accumulated: Duration::default(),
            last: Instant::now(),
        }
    }
    pub fn check<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let res = f();
        let end = Instant::now();
        let since_last = start.duration_since(self.last);
        let tolerance = since_last / 1000;
        let duration =end.duration_since(start);
        if duration > tolerance {
            self.accumulated += duration;
        }
        self.last = end;
        if self.accumulated > Duration::from_secs(30) {
            info!("Bottleneck: {}", self.name);
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
