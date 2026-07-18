//! Opaque JSON values retained in explicit extension namespaces.

use std::{
    collections::{BTreeMap, btree_map::Entry},
    error::Error,
    fmt,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use serde_json::value::RawValue;

/// A valid JSON value retained without interpreting or normalizing its contents.
///
/// Equality intentionally compares the retained raw JSON text. This keeps a canonical request
/// from silently changing an opaque extension's representation.
#[derive(Clone)]
pub struct RawJson(Box<RawValue>);

impl RawJson {
    /// Parses one complete JSON value and retains its original representation.
    ///
    /// # Errors
    ///
    /// Returns [`serde_json::Error`] when `value` is not one complete JSON value.
    pub fn from_json_string(value: String) -> Result<Self, serde_json::Error> {
        RawValue::from_string(value).map(Self)
    }

    /// Returns the retained JSON text without parsing or normalizing it.
    #[must_use]
    pub fn get(&self) -> &str {
        self.0.get()
    }
}

impl fmt::Debug for RawJson {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RawJson(<opaque>)")
    }
}

impl PartialEq for RawJson {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl Eq for RawJson {}

impl Serialize for RawJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RawJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Box::<RawValue>::deserialize(deserializer).map(Self)
    }
}

impl From<Box<RawValue>> for RawJson {
    fn from(value: Box<RawValue>) -> Self {
        Self(value)
    }
}

/// Error returned when a raw extension namespace would contain the same field more than once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawExtensionError {
    /// A second value was supplied for an existing extension field name.
    DuplicateName,
}

impl fmt::Display for RawExtensionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateName => formatter.write_str("raw extension field name already exists"),
        }
    }
}

impl Error for RawExtensionError {}

/// Provider- or protocol-specific fields stored only under an explicit `extensions` field.
///
/// The internal `BTreeMap` provides deterministic key ordering while each value remains raw JSON.
/// Duplicate JSON field names are rejected during deserialization instead of silently allowing a
/// later value to replace an earlier unknown field.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct RawExtensions(BTreeMap<String, RawJson>);

impl RawExtensions {
    /// Returns the raw extension value stored under `name`, if present.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&RawJson> {
        self.0.get(name)
    }

    /// Inserts one raw extension if `name` is not already present.
    ///
    /// # Errors
    ///
    /// Returns [`RawExtensionError::DuplicateName`] when `name` is already present. Existing
    /// data is retained unchanged in that case.
    pub fn try_insert(
        &mut self,
        name: impl Into<String>,
        value: RawJson,
    ) -> Result<(), RawExtensionError> {
        match self.0.entry(name.into()) {
            Entry::Vacant(entry) => {
                entry.insert(value);
                Ok(())
            }
            Entry::Occupied(_) => Err(RawExtensionError::DuplicateName),
        }
    }

    /// Iterates over extension fields in deterministic lexicographic key order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &RawJson)> {
        self.0.iter().map(|(name, value)| (name.as_str(), value))
    }

    /// Returns whether this extension namespace contains no fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for RawExtensions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "RawExtensions(<{} fields>)", self.0.len())
    }
}

impl Serialize for RawExtensions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RawExtensions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RawExtensionsVisitor;

        impl<'de> de::Visitor<'de> for RawExtensionsVisitor {
            type Value = RawExtensions;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object containing unique raw extension fields")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((name, value)) = map.next_entry::<String, RawJson>()? {
                    if values.contains_key(&name) {
                        return Err(de::Error::custom(
                            "raw extensions must not contain duplicate field names",
                        ));
                    }
                    values.insert(name, value);
                }

                Ok(RawExtensions(values))
            }
        }

        deserializer.deserialize_map(RawExtensionsVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::{RawExtensionError, RawExtensions, RawJson};

    #[test]
    fn raw_json_keeps_the_original_json_text() -> Result<(), serde_json::Error> {
        let raw = RawJson::from_json_string("{ \"flag\" : true }".to_owned())?;

        assert_eq!(raw.get(), "{ \"flag\" : true }");
        assert_eq!(serde_json::to_string(&raw)?, "{ \"flag\" : true }");

        Ok(())
    }

    #[test]
    fn raw_json_rejects_an_incomplete_value() {
        assert!(RawJson::from_json_string("{\"flag\":".to_owned()).is_err());
    }

    #[test]
    fn raw_extensions_reject_duplicate_names() {
        let decoded = serde_json::from_str::<super::RawExtensions>(
            r#"{"vendor":{"first":true},"vendor":{"second":true}}"#,
        );

        assert!(decoded.is_err());
    }

    #[test]
    fn raw_extensions_support_lossless_construction_enumeration_and_round_trip()
    -> Result<(), serde_json::Error> {
        let mut extensions = RawExtensions::default();
        let alpha = RawJson::from_json_string(r#"{"enabled":true}"#.to_owned())?;
        let beta = RawJson::from_json_string(r#"["preserve",2]"#.to_owned())?;

        assert!(extensions.try_insert("beta", beta).is_ok());
        assert!(extensions.try_insert("alpha", alpha).is_ok());
        assert_eq!(
            extensions
                .iter()
                .map(|(name, value)| (name, value.get()))
                .collect::<Vec<_>>(),
            vec![
                ("alpha", r#"{"enabled":true}"#),
                ("beta", r#"["preserve",2]"#)
            ]
        );
        assert!(matches!(
            extensions.try_insert("alpha", RawJson::from_json_string("null".to_owned())?),
            Err(RawExtensionError::DuplicateName)
        ));

        let encoded = serde_json::to_string(&extensions)?;
        let restored: RawExtensions = serde_json::from_str(&encoded)?;

        assert_eq!(extensions, restored);
        assert_eq!(
            encoded,
            r#"{"alpha":{"enabled":true},"beta":["preserve",2]}"#
        );
        Ok(())
    }
}
