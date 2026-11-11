use crate::serde_path_to_error::{Chain, Track};
pub struct Wrap<'a, 'b, X> {
    pub(crate) delegate: X,
    pub(crate) chain: &'a Chain<'a>,
    pub(crate) track: &'b Track,
}
pub struct WrapVariant<'a, 'b, X> {
    pub(crate) delegate: X,
    pub(crate) chain: Chain<'a>,
    pub(crate) track: &'b Track,
}
impl<'a, 'b, X> Wrap<'a, 'b, X> {
    pub(crate) fn new(delegate: X, chain: &'a Chain<'a>, track: &'b Track) -> Self {
        Wrap {
            delegate,
            chain,
            track,
        }
    }
}
impl<'a, 'b, X> WrapVariant<'a, 'b, X> {
    pub(crate) fn new(delegate: X, chain: Chain<'a>, track: &'b Track) -> Self {
        WrapVariant {
            delegate,
            chain,
            track,
        }
    }
}
