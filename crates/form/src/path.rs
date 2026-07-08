use serde::{Deserialize, Serialize};

/// Stable path for a form field.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FieldPath(String);

impl FieldPath {
    /// Creates a field path from a non-empty string.
    pub fn new(path: impl Into<String>) -> Result<Self, FieldPathError> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(FieldPathError::Empty);
        }
        Ok(Self(path))
    }

    /// Returns the path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for FieldPath {
    type Error = FieldPathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for FieldPath {
    type Error = FieldPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl std::fmt::Display for FieldPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable field identity used by snapshots and adapters.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FieldId(String);

impl FieldId {
    /// Creates a field id from a non-empty string.
    pub fn new(id: impl Into<String>) -> Result<Self, FieldPathError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(FieldPathError::Empty);
        }
        Ok(Self(id))
    }

    /// Returns the id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&FieldPath> for FieldId {
    fn from(path: &FieldPath) -> Self {
        Self(path.as_str().to_owned())
    }
}

impl std::fmt::Display for FieldId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error returned when a field path or id is invalid.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FieldPathError {
    /// The supplied path or id was empty.
    #[error("field path cannot be empty")]
    Empty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_path_rejects_empty_values() {
        assert_eq!(FieldPath::new(""), Err(FieldPathError::Empty));
        assert_eq!(FieldId::new("  "), Err(FieldPathError::Empty));
    }

    #[test]
    fn field_id_can_be_derived_from_path() {
        let path = FieldPath::new("account.email").unwrap();
        let id = FieldId::from(&path);

        assert_eq!(id.as_str(), "account.email");
    }
}
