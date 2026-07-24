use std::{collections::BTreeSet, fmt};

use gateway_core::{ErrorScope, GatewayError, GatewayErrorCode, RawExtensions, RawJson};
use serde::{Deserialize, de};
use serde_json::{Map, Value};

pub(crate) fn reject_duplicate_names(input: &str) -> Result<(), GatewayError> {
    serde_json::from_str::<DuplicateFreeJson>(input)
        .map(|_| ())
        .map_err(|_| client_request_error())
}

pub(crate) fn object(value: &Value) -> Result<&Map<String, Value>, GatewayError> {
    value.as_object().ok_or_else(client_request_error)
}

pub(crate) fn array(value: &Value) -> Result<&Vec<Value>, GatewayError> {
    value.as_array().ok_or_else(client_request_error)
}

pub(crate) fn required_value<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a Value, GatewayError> {
    object.get(name).ok_or_else(client_request_error)
}

pub(crate) fn required_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, GatewayError> {
    required_value(object, name)?
        .as_str()
        .ok_or_else(client_request_error)
}

pub(crate) fn raw_json(value: &Value) -> Result<RawJson, GatewayError> {
    let encoded = serde_json::to_string(value).map_err(|_| internal_error())?;
    RawJson::from_json_string(encoded).map_err(|_| internal_error())
}

pub(crate) fn extensions_except(
    object: &Map<String, Value>,
    known: &[&str],
    prefix: &str,
) -> Result<RawExtensions, GatewayError> {
    let known: BTreeSet<&str> = known.iter().copied().collect();
    let mut extensions = RawExtensions::default();
    for (name, value) in object {
        if !known.contains(name.as_str()) {
            extensions
                .try_insert(format!("{prefix}{name}"), raw_json(value)?)
                .map_err(|_| client_request_error())?;
        }
    }
    Ok(extensions)
}

pub(crate) const fn client_request_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

pub(crate) const fn internal_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::InternalError, ErrorScope::Internal)
}

pub(crate) const fn stream_protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

struct DuplicateFreeJson;

impl<'de> Deserialize<'de> for DuplicateFreeJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateFreeVisitor)
    }
}

struct DuplicateFreeVisitor;

impl<'de> de::Visitor<'de> for DuplicateFreeVisitor {
    type Value = DuplicateFreeJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object member names")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateFreeJson)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        while sequence.next_element::<DuplicateFreeJson>()?.is_some() {}
        Ok(DuplicateFreeJson)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut names = BTreeSet::new();
        while let Some(name) = map.next_key::<String>()? {
            if !names.insert(name) {
                return Err(de::Error::custom("duplicate JSON object member name"));
            }
            let _value = map.next_value::<DuplicateFreeJson>()?;
        }
        Ok(DuplicateFreeJson)
    }
}
