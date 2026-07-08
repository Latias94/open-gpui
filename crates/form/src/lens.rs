use serde_json::Value;

use crate::{FieldPath, FormError};

/// Typed lens between app-owned values and renderer-neutral field values.
pub struct FieldLens<T> {
    path: FieldPath,
    get: Box<dyn Fn(&T) -> Value + Send + Sync>,
    set: Box<dyn Fn(&mut T, Value) -> Result<(), FormError> + Send + Sync>,
}

impl<T> FieldLens<T> {
    /// Creates a typed field lens.
    pub fn new(
        path: FieldPath,
        get: impl Fn(&T) -> Value + Send + Sync + 'static,
        set: impl Fn(&mut T, Value) -> Result<(), FormError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            path,
            get: Box::new(get),
            set: Box::new(set),
        }
    }

    /// Returns the field path this lens owns.
    pub fn path(&self) -> &FieldPath {
        &self.path
    }

    /// Reads the app-owned value through the lens.
    pub fn get(&self, source: &T) -> Value {
        (self.get)(source)
    }

    /// Writes the app-owned value through the lens.
    pub fn set(&self, source: &mut T, value: Value) -> Result<(), FormError> {
        (self.set)(source, value)
    }
}
