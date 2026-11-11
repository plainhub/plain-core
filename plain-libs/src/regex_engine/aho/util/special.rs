use crate::regex_engine::aho::util::primitives::StateID;
#[derive(Clone, Debug)]
pub(crate) struct Special {
    pub(crate) max_special_id: StateID,
    pub(crate) max_match_id: StateID,
    pub(crate) start_unanchored_id: StateID,
    pub(crate) start_anchored_id: StateID,
}
impl Special {
    pub(crate) fn zero() -> Special {
        Special {
            max_special_id: StateID::ZERO,
            max_match_id: StateID::ZERO,
            start_unanchored_id: StateID::ZERO,
            start_anchored_id: StateID::ZERO,
        }
    }
}
