use std::{error::Error, fmt};

use crate::{CanvasEvent, CanvasToolId, DocumentError};
use indexmap::IndexMap;

use super::{CanvasToolContext, CanvasToolIntent};

pub trait CanvasToolReducer {
    fn handle_event(
        &mut self,
        context: CanvasToolContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolIntent>, DocumentError>;
}

impl<F> CanvasToolReducer for F
where
    F: for<'a> FnMut(
        CanvasToolContext<'a>,
        CanvasEvent,
    ) -> Result<Vec<CanvasToolIntent>, DocumentError>,
{
    fn handle_event(
        &mut self,
        context: CanvasToolContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolIntent>, DocumentError> {
        self(context, event)
    }
}

#[derive(Default)]
pub struct CanvasToolRegistry {
    reducers: IndexMap<CanvasToolId, Box<dyn CanvasToolReducer>>,
}

impl CanvasToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T>(
        &mut self,
        id: impl Into<CanvasToolId>,
        reducer: T,
    ) -> Option<Box<dyn CanvasToolReducer>>
    where
        T: CanvasToolReducer + 'static,
    {
        self.reducers.insert(id.into(), Box::new(reducer))
    }

    pub fn insert_boxed(
        &mut self,
        id: impl Into<CanvasToolId>,
        reducer: Box<dyn CanvasToolReducer>,
    ) -> Option<Box<dyn CanvasToolReducer>> {
        self.reducers.insert(id.into(), reducer)
    }

    pub fn remove(&mut self, id: &CanvasToolId) -> Option<Box<dyn CanvasToolReducer>> {
        self.reducers.shift_remove(id)
    }

    pub fn contains(&self, id: &CanvasToolId) -> bool {
        self.reducers.contains_key(id)
    }

    pub fn reducer_mut(&mut self, id: &CanvasToolId) -> Option<&mut (dyn CanvasToolReducer + '_)> {
        let reducer = self.reducers.get_mut(id)?;
        Some(reducer.as_mut())
    }

    pub fn ids(&self) -> impl Iterator<Item = &CanvasToolId> {
        self.reducers.keys()
    }

    pub fn len(&self) -> usize {
        self.reducers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.reducers.is_empty()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CanvasToolRegistryError {
    MissingTool(CanvasToolId),
    Document(DocumentError),
}

impl From<DocumentError> for CanvasToolRegistryError {
    fn from(value: DocumentError) -> Self {
        Self::Document(value)
    }
}

impl fmt::Display for CanvasToolRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTool(id) => write!(f, "canvas custom tool `{id}` is not registered"),
            Self::Document(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl Error for CanvasToolRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingTool(_) => None,
            Self::Document(error) => Some(error),
        }
    }
}
