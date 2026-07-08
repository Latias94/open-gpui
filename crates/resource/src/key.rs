use serde::{Deserialize, Serialize};

/// One segment of a query key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum QueryKeySegment {
    /// Text segment.
    Text(String),
    /// Signed integer segment.
    Integer(i64),
    /// Boolean segment.
    Bool(bool),
}

impl From<&str> for QueryKeySegment {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for QueryKeySegment {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<i64> for QueryKeySegment {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<bool> for QueryKeySegment {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// Deterministic query key used by resource caches and observers.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct QueryKey(Vec<QueryKeySegment>);

impl QueryKey {
    /// Creates a non-empty query key.
    pub fn new(
        segments: impl IntoIterator<Item = impl Into<QueryKeySegment>>,
    ) -> Result<Self, QueryKeyError> {
        let segments = segments.into_iter().map(Into::into).collect::<Vec<_>>();
        if segments.is_empty() {
            return Err(QueryKeyError::Empty);
        }
        Ok(Self(segments))
    }

    /// Returns the key segments.
    pub fn segments(&self) -> &[QueryKeySegment] {
        &self.0
    }
}

/// Error returned when a query key is invalid.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum QueryKeyError {
    /// Query keys need at least one segment.
    #[error("query key cannot be empty")]
    Empty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_key_rejects_empty_keys() {
        let result = QueryKey::new(Vec::<QueryKeySegment>::new());

        assert_eq!(result, Err(QueryKeyError::Empty));
    }

    #[test]
    fn query_key_preserves_segments() {
        let key = QueryKey::new(["workspace", "members"]).unwrap();

        assert_eq!(key.segments().len(), 2);
    }
}
