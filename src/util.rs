pub mod bitcoin;

pub fn reversed<I, II, IT>(i: I) -> I
where
    I: IntoIterator<IntoIter = II, Item = IT>,
    II: DoubleEndedIterator<Item = IT>,
    I: std::iter::FromIterator<IT>,
{
    i.into_iter().rev().collect()
}
