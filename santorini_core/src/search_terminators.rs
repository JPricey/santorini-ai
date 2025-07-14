use std::marker::PhantomData;

use crate::search::SearchState;

/// Trait to check if a search should stop at some static boundary
pub trait SearchTerminator {
    fn should_stop(search_state: &SearchState) -> bool;
}

pub struct NoopSearchTerminator {}

impl SearchTerminator for NoopSearchTerminator {
    fn should_stop(_search_state: &SearchState) -> bool {
        false
    }
}

pub struct StaticMaxDepthSearchTerminator<const N: usize> {}
impl<const N: usize> SearchTerminator for StaticMaxDepthSearchTerminator<N> {
    fn should_stop(search_state: &SearchState) -> bool {
        search_state.last_fully_completed_depth >= N
    }
}

pub struct StaticNodesVisitedSearchTerminator<const N: usize> {}
impl<const N: usize> SearchTerminator for StaticNodesVisitedSearchTerminator<N> {
    fn should_stop(search_state: &SearchState) -> bool {
        search_state.nodes_visited >= N
    }
}

pub struct AndSearchTerminator<A: SearchTerminator, B: SearchTerminator> {
    a_type: PhantomData<A>,
    b_type: PhantomData<B>,
}
impl<A: SearchTerminator, B: SearchTerminator> SearchTerminator
    for AndSearchTerminator<A, B>
{
    fn should_stop(search_state: &SearchState) -> bool {
        A::should_stop(search_state) && B::should_stop(search_state)
    }
}

pub struct OrSearchTerminator<A: SearchTerminator, B: SearchTerminator> {
    a_type: PhantomData<A>,
    b_type: PhantomData<B>,
}
impl<A: SearchTerminator, B: SearchTerminator> SearchTerminator
    for OrSearchTerminator<A, B>
{
    fn should_stop(search_state: &SearchState) -> bool {
        A::should_stop(search_state) || B::should_stop(search_state)
    }
}

