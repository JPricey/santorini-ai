use crate::search::SearchState;

/// Trait to check if a search should stop at some static boundary
pub trait SearchTerminator {
    fn should_stop(&self, search_state: &SearchState) -> bool;
}

#[derive(Default)]
pub struct NoopSearchTerminator {}

impl SearchTerminator for NoopSearchTerminator {
    fn should_stop(&self, _search_state: &SearchState) -> bool {
        false
    }
}

#[derive(Default)]
pub struct StaticMaxDepthSearchTerminator<const N: usize> {}
impl<const N: usize> SearchTerminator for StaticMaxDepthSearchTerminator<N> {
    fn should_stop(&self, search_state: &SearchState) -> bool {
        search_state.last_fully_completed_depth >= N
    }
}

pub struct DynamicMaxDepthSearchTerminator {
    pub max_depth: usize,
}
impl SearchTerminator for DynamicMaxDepthSearchTerminator {
    fn should_stop(&self, search_state: &SearchState) -> bool {
        search_state.last_fully_completed_depth >= self.max_depth
    }
}
impl DynamicMaxDepthSearchTerminator {
    pub fn new(max_depth: usize) -> Self {
        DynamicMaxDepthSearchTerminator { max_depth }
    }
}

#[derive(Default)]
pub struct StaticNodesVisitedSearchTerminator<const N: usize> {}
impl<const N: usize> SearchTerminator for StaticNodesVisitedSearchTerminator<N> {
    fn should_stop(&self, search_state: &SearchState) -> bool {
        search_state.nodes_visited >= N
    }
}

pub struct AndSearchTerminator<A: SearchTerminator, B: SearchTerminator> {
    a: A,
    b: B,
}
impl<A: SearchTerminator, B: SearchTerminator> SearchTerminator for AndSearchTerminator<A, B> {
    fn should_stop(&self, search_state: &SearchState) -> bool {
        self.a.should_stop(search_state) && self.b.should_stop(search_state)
    }
}

impl<A, B> Default for AndSearchTerminator<A, B>
where
    A: SearchTerminator + Default,
    B: SearchTerminator + Default,
{
    fn default() -> Self {
        Self {
            a: Default::default(),
            b: Default::default(),
        }
    }
}

pub struct OrSearchTerminator<A: SearchTerminator, B: SearchTerminator> {
    a: A,
    b: B,
}
impl<A: SearchTerminator, B: SearchTerminator> SearchTerminator for OrSearchTerminator<A, B> {
    fn should_stop(&self, search_state: &SearchState) -> bool {
        self.a.should_stop(search_state) || self.b.should_stop(search_state)
    }
}

impl<A, B> Default for OrSearchTerminator<A, B>
where
    A: SearchTerminator + Default,
    B: SearchTerminator + Default,
{
    fn default() -> Self {
        Self {
            a: Default::default(),
            b: Default::default(),
        }
    }
}
