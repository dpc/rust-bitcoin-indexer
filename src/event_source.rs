use crate::{prelude::*, WithHeightAndId};

/// Block Event source...
///
/// ...that can be followed by keeping a `Cursor` and periodically
/// polling for new data.
pub trait EventSource {
    type Cursor;
    type Id;
    type Data;

    /// Poll for events after position indicated by `cursor`,
    ///
    /// Returns new elements and new `Cursor` position.
    fn next(
        &mut self,
        cursor: Option<Self::Cursor>,
        limit: u64,
    ) -> Result<(Vec<WithHeightAndId<Self::Id, Self::Data>>, Self::Cursor)>;
}
