use std::error::Error;

use crate::{ApplyOutcome, Event, RenderModel, RenderOutput, ResolvedTabName, State};

pub trait Renderer {
    type Error: Error + Send + Sync + 'static;

    fn render(
        &self,
        model: &RenderModel,
        available_width: usize,
    ) -> Result<RenderOutput, Self::Error>;
}

pub trait StatusStore {
    type Error: Error + Send + Sync + 'static;

    fn state(&self) -> &State;
    fn apply(&mut self, event: Event) -> Result<ApplyOutcome, Self::Error>;
}

pub trait DirectoryResolver {
    type Error: Error + Send + Sync + 'static;

    fn resolve(&self, state: &State) -> Result<Vec<ResolvedTabName>, Self::Error>;
}

pub trait Transport {
    type Error: Error + Send + Sync + 'static;

    fn send(&mut self, event: &Event) -> Result<(), Self::Error>;
}

impl StatusStore for State {
    type Error = crate::StateError;

    fn state(&self) -> &State {
        self
    }

    fn apply(&mut self, event: Event) -> Result<ApplyOutcome, Self::Error> {
        State::apply(self, event)
    }
}
