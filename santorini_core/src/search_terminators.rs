use std::marker::PhantomData;

use crate::search::SearchState;

/// Trait to check if a search should stop at some static boundary
pub trait StaticSearchTerminator {
    fn should_stop(search_state: &SearchState) -> bool;
}

pub struct NoopStaticSearchTerminator {}

impl StaticSearchTerminator for NoopStaticSearchTerminator {
    fn should_stop(_search_state: &SearchState) -> bool {
        false
    }
}

pub struct MaxDepthStaticSearchTerminator<const N: usize> {}
impl<const N: usize> StaticSearchTerminator for MaxDepthStaticSearchTerminator<N> {
    fn should_stop(search_state: &SearchState) -> bool {
        search_state.last_fully_completed_depth >= N
    }
}

pub struct NodesVisitedStaticSearchTerminator<const N: usize> {}
impl<const N: usize> StaticSearchTerminator for NodesVisitedStaticSearchTerminator<N> {
    fn should_stop(search_state: &SearchState) -> bool {
        search_state.nodes_visited >= N
    }
}

pub struct AndStaticSearchTerminator<A: StaticSearchTerminator, B: StaticSearchTerminator> {
    a_type: PhantomData<A>,
    b_type: PhantomData<B>,
}
impl<A: StaticSearchTerminator, B: StaticSearchTerminator> StaticSearchTerminator
    for AndStaticSearchTerminator<A, B>
{
    fn should_stop(search_state: &SearchState) -> bool {
        A::should_stop(search_state) && B::should_stop(search_state)
    }
}

pub struct OrStaticSearchTerminator<A: StaticSearchTerminator, B: StaticSearchTerminator> {
    a_type: PhantomData<A>,
    b_type: PhantomData<B>,
}
impl<A: StaticSearchTerminator, B: StaticSearchTerminator> StaticSearchTerminator
    for OrStaticSearchTerminator<A, B>
{
    fn should_stop(search_state: &SearchState) -> bool {
        A::should_stop(search_state) || B::should_stop(search_state)
    }
}

