//! Opaque identifiers used to keep gateway-domain boundaries explicit.

use std::{error::Error, fmt};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Error returned when an opaque gateway identifier is empty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidIdentifier {
    /// The supplied identifier did not contain any bytes.
    Empty,
}

impl fmt::Display for InvalidIdentifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("gateway identifier must not be empty"),
        }
    }
}

impl Error for InvalidIdentifier {}

macro_rules! opaque_identifier {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Creates an identifier while preserving its supplied opaque representation.
            ///
            /// # Errors
            ///
            /// Returns [`InvalidIdentifier::Empty`] when `value` is empty.
            pub fn try_new(value: impl Into<String>) -> Result<Self, InvalidIdentifier> {
                let value = value.into();
                if value.is_empty() {
                    return Err(InvalidIdentifier::Empty);
                }

                Ok(Self(value))
            }

            /// Returns the opaque representation without changing or parsing it.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidIdentifier;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = InvalidIdentifier;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::try_new(value).map_err(de::Error::custom)
            }
        }
    };
}

opaque_identifier!(
    RequestId,
    "Stable identifier for one externally accepted gateway request."
);
opaque_identifier!(
    ResponseId,
    "Stable identifier for one client-visible canonical response."
);
opaque_identifier!(
    AttemptId,
    "Stable identifier for one concrete upstream attempt within a request."
);
opaque_identifier!(
    ClientKeyId,
    "Stable identifier for a client API key without exposing its secret material."
);
opaque_identifier!(
    AccessGroupId,
    "Stable identifier for a client access group."
);
opaque_identifier!(
    AuthId,
    "Stable identifier for the final authentication material selected for an attempt."
);
opaque_identifier!(
    CredentialId,
    "Stable identifier for an encrypted upstream credential record."
);
opaque_identifier!(
    ProviderId,
    "Stable identifier for a provider implementation family."
);
opaque_identifier!(
    UpstreamId,
    "Stable identifier for a configured upstream instance."
);
opaque_identifier!(
    EndpointId,
    "Stable identifier for one protocol-specific upstream endpoint."
);
opaque_identifier!(
    PublicModelId,
    "Stable identifier for a client-visible public model."
);
opaque_identifier!(
    RouteId,
    "Stable identifier for a public-model routing policy."
);
opaque_identifier!(
    RouteCandidateId,
    "Stable identifier for one concrete route candidate."
);

#[cfg(test)]
mod tests {
    use super::{InvalidIdentifier, RequestId, ResponseId};

    #[test]
    fn identifiers_preserve_a_non_empty_opaque_value() {
        let result = RequestId::try_new("request-01");

        assert!(result.is_ok());
        if let Ok(request_id) = result {
            assert_eq!(request_id.as_str(), "request-01");
            assert_eq!(request_id.to_string(), "request-01");
        }
    }

    #[test]
    fn identifiers_reject_an_empty_value() {
        assert_eq!(RequestId::try_new(""), Err(InvalidIdentifier::Empty));
    }

    #[test]
    fn response_identifier_json_round_trip_preserves_the_non_empty_invariant()
    -> Result<(), serde_json::Error> {
        let result = ResponseId::try_new("response-01");

        assert!(result.is_ok());
        if let Ok(response_id) = result {
            let encoded = serde_json::to_string(&response_id)?;
            let decoded: ResponseId = serde_json::from_str(&encoded)?;

            assert_eq!(encoded, r#""response-01""#);
            assert_eq!(decoded, response_id);
        }
        assert!(serde_json::from_str::<ResponseId>(r#""""#).is_err());

        Ok(())
    }
}
