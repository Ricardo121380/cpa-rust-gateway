//! Explicit, immutable authority for one Build model relationship. No inventory or I/O.
use std::fmt;

use gateway_core::{ClientKeyId, GatewayError, ModelIdentityRejection, RequestId};
use gateway_router::{ProtocolFormat, ResponsesExecutionLineage};
use sha2::{Digest, Sha256};

use crate::GrokBuildExecutionMode;

/// Exact applicability of independently approved alias evidence, frozen before dispatch.
#[derive(Clone, Eq, PartialEq)]
pub struct GrokBuildModelAliasScope {
    /// Immutable configuration, route, candidate, channel and actual credential/revision.
    pub lineage: ResponsesExecutionLineage,
    /// Revision of that immutable configuration graph.
    pub config_revision: i64,
    /// Credential channel, independently of the configured endpoint ID.
    pub channel: String,
    /// Logical request owner; a management Pin may have no Client Key.
    pub client_key_id: Option<ClientKeyId>,
    /// Actual downstream protocol, not inferred from a client name.
    pub client_protocol: ProtocolFormat,
    /// Actual JSON/SSE execution mode.
    pub mode: GrokBuildExecutionMode,
    /// Frozen Build wire-profile version.
    pub client_profile: String,
}

impl fmt::Debug for GrokBuildModelAliasScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokBuildModelAliasScope(<protected identifiers>)")
    }
}

/// Authority must be reviewed independently of routing/catalog configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildAliasAuthority {
    /// An authoritative, version-specific model statement.
    OfficialVersionStatement,
    /// An independently trusted serving declaration for the exact execution scope.
    ServingDeclaration,
    /// Separately approved verification material; tests explicitly label it synthetic.
    ApprovedVerification,
}

/// Assertions extracted from already approved evidence by the composition owner.
/// Neither a hash nor this record establishes the trustworthiness of its source.
#[derive(Clone)]
pub struct GrokBuildModelAliasAssertion {
    /// Admitted public model, after ordinary route-alias resolution.
    pub public_model: String,
    /// Model selected by the actual candidate and sent on the wire.
    pub upstream_model: String,
    /// The one native model that this proof permits.
    pub native_model: String,
    /// Independently reviewed authority category.
    pub authority: GrokBuildAliasAuthority,
    /// Frozen source locator or explicit approval reference; never fetched here.
    pub source_reference: String,
    /// SHA256 of the exact source material supplied to `freeze`.
    pub source_sha256: String,
    /// Inclusive validity start in Unix milliseconds.
    pub valid_from_ms: i64,
    /// Exclusive expiry in Unix milliseconds; never renewed by a response.
    pub valid_until_ms: i64,
}

impl fmt::Debug for GrokBuildModelAliasAssertion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokBuildModelAliasAssertion(<approved evidence>)")
    }
}

/// A reviewed assertion and its exact immutable applicability; no setters or deserializer.
#[derive(Clone)]
pub struct GrokBuildModelAliasProof {
    scope: GrokBuildModelAliasScope,
    assertion: GrokBuildModelAliasAssertion,
}

impl GrokBuildModelAliasProof {
    /// Freezes independently approved material before any request using it is dispatched.
    ///
    /// The caller is the trust boundary: approve the source and its assertions independently.
    /// Route aliases, a current catalog, or one native response cannot supply that approval.
    /// Source bytes are hash-checked and then discarded; this performs no network or file read.
    ///
    /// # Errors
    /// Rejects malformed bounds, source-digest mismatch and invalid validity intervals.
    pub fn freeze(
        scope: GrokBuildModelAliasScope,
        assertion: GrokBuildModelAliasAssertion,
        approved_source: &[u8],
    ) -> Result<Self, GatewayError> {
        let digest = format!("{:x}", Sha256::digest(approved_source));
        if approved_source.is_empty()
            || approved_source.len() > 65_536
            || assertion.source_sha256 != digest
            || assertion.source_reference.is_empty()
            || assertion.source_reference.len() > 2048
            || !valid_model(&assertion.public_model)
            || !valid_model(&assertion.upstream_model)
            || !valid_model(&assertion.native_model)
            || !valid_model(&scope.channel)
            || !valid_model(&scope.client_profile)
            || scope.config_revision < 0
            || assertion.valid_from_ms < 0
            || assertion.valid_until_ms <= assertion.valid_from_ms
        {
            return Err(GatewayError::model_identity(
                ModelIdentityRejection::AliasNotApplicable,
            ));
        }
        Ok(Self { scope, assertion })
    }
}

impl fmt::Debug for GrokBuildModelAliasProof {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokBuildModelAliasProof(<frozen approved evidence>)")
    }
}

/// The pre-dispatch public/upstream proof and the one expected native identity.
#[derive(Clone)]
pub struct GrokBuildModelBinding {
    upstream_model: String,
    native_model: String,
    request_id: Option<RequestId>,
}

impl GrokBuildModelBinding {
    /// Admits literal equality without inventing an alias from configuration.
    ///
    /// # Errors
    /// Rejects an invalid identifier or unequal public/upstream names before sending.
    pub fn literal(public_model: &str, upstream_model: &str) -> Result<Self, GatewayError> {
        if public_model != upstream_model || !valid_model(upstream_model) {
            return Err(GatewayError::model_identity(
                ModelIdentityRejection::RequestMismatch,
            ));
        }
        Ok(Self {
            upstream_model: upstream_model.to_owned(),
            native_model: upstream_model.to_owned(),
            request_id: None,
        })
    }

    /// Resolves only pre-frozen authority for this exact actual execution, before dispatch.
    ///
    /// # Errors
    /// Rejects expired, foreign, ambiguous or otherwise inapplicable proof. Literal equality
    /// is considered only when no proof claims this public/upstream pair.
    pub fn for_execution(
        scope: &GrokBuildModelAliasScope,
        public_model: &str,
        upstream_model: &str,
        proofs: &[GrokBuildModelAliasProof],
        now_ms: i64,
        request_id: &RequestId,
    ) -> Result<Self, GatewayError> {
        let mut claiming = false;
        let mut native: Option<&str> = None;
        for proof in proofs.iter().filter(|proof| {
            proof.assertion.public_model == public_model
                && proof.assertion.upstream_model == upstream_model
        }) {
            claiming = true;
            if &proof.scope != scope
                || scope.channel != "grok.build"
                || now_ms < proof.assertion.valid_from_ms
                || now_ms >= proof.assertion.valid_until_ms
            {
                continue;
            }
            if native.is_some_and(|previous| previous != proof.assertion.native_model) {
                return Err(GatewayError::model_identity(
                    ModelIdentityRejection::AliasNotApplicable,
                ));
            }
            native = Some(&proof.assertion.native_model);
        }
        if let Some(native_model) = native {
            return Ok(Self {
                upstream_model: upstream_model.to_owned(),
                native_model: native_model.to_owned(),
                request_id: Some(request_id.clone()),
            });
        }
        if claiming {
            return Err(GatewayError::model_identity(
                ModelIdentityRejection::AliasNotApplicable,
            ));
        }
        Self::literal(public_model, upstream_model)
    }

    pub(crate) fn expected_native_model(
        &self,
        actual_upstream: &str,
        request_id: &RequestId,
    ) -> Result<String, GatewayError> {
        if self.upstream_model != actual_upstream
            || self
                .request_id
                .as_ref()
                .is_some_and(|expected| expected != request_id)
        {
            return Err(GatewayError::model_identity(
                ModelIdentityRejection::RequestMismatch,
            ));
        }
        Ok(self.native_model.clone())
    }
}

impl fmt::Debug for GrokBuildModelBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrokBuildModelBinding(<validated relationship>)")
    }
}

pub(crate) fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 512
        && model.trim() == model
        && !model.chars().any(char::is_control)
}
