//! Management views and explicit recovery over the active serving generation.

use super::{
    Arc, AttemptExclusionSet, AttemptOutcome, BTreeMap, BTreeSet, CatalogDiscoveryFailureClass,
    CatalogModelState, CatalogSnapshotFreshness, ConfigRevision, EndpointId, GatewayEvent,
    ManagementCatalogFailureClass, ManagementCatalogFreshness, ManagementCatalogStatus,
    ManagementQuotaRecoveryState, ManagementRequestAttempt, ManagementRequestProtocol,
    ManagementRouteExplain, ManagementRouteExplainCandidate, ManagementRouteExplainPricePolicy,
    ManagementRouteExplainRequest, ManagementRuntimeAvailabilityStatus, ManagementRuntimeError,
    ManagementRuntimeFacade, ManagementRuntimeTarget, ModelCatalogTarget, P12AttemptStageStore,
    ProtocolFormat, ProviderId, ProviderScopedPriceEvidence, ProviderScopedRouteExplainInput,
    ProviderScopedRouteExplainSnapshot, QuotaConfidence, QuotaSnapshot, QuotaSource,
    RouteCredentialScheduler, RouteExplainCandidate, RouteExplainCandidateReason,
    RouteExplainInput, RouteSnapshot, RouteSnapshotRegistry, RoutingPriceComparison,
    RoutingPriceSnapshot, RuntimeCredentialAccountStatus, RuntimeHealthAccountRecoveryResult,
    RuntimeHealthRegistry, RuntimeQuotaAvailability, RuntimeQuotaRegistry, RuntimeQuotaTarget,
    SnapshotRouteCandidate, SnapshotVersion, SqliteCatalogSnapshotStore, SqliteEventStore,
    execution_identity, protocol_pair_is_publishable,
};

/// Lifetime of one operator-driven recovery ticket; begin and complete happen in one call.
const P12_OPERATOR_RECOVERY_TTL_MS: i64 = 30_000;

pub(super) struct SnapshotManagementRuntimeFacade {
    pub(super) execution_configuration: Option<(SnapshotVersion, ConfigRevision)>,
    pub(super) execution_owned_reasoning: BTreeSet<EndpointId>,
    pub(super) registry: Arc<RouteSnapshotRegistry>,
    pub(super) attempt_stages: Arc<P12AttemptStageStore>,
    pub(super) runtime_health: Arc<RuntimeHealthRegistry>,
    pub(super) runtime_quota: Arc<RuntimeQuotaRegistry>,
    pub(super) route_explain_scheduler: Option<Arc<RouteCredentialScheduler>>,
    pub(super) routing_price_snapshot: Option<Arc<RoutingPriceSnapshot>>,
    pub(super) event_store: SqliteEventStore,
    pub(super) catalog_store: SqliteCatalogSnapshotStore,
    pub(super) catalog_targets: Vec<(ModelCatalogTarget, bool)>,
}

impl SnapshotManagementRuntimeFacade {
    pub(super) fn snapshot_for(
        &self,
        version_id: &gateway_store::control_plane::ConfigVersionId,
    ) -> Result<Arc<RouteSnapshot>, ManagementRuntimeError> {
        // Match data-plane authentication, including same-version discovery publications.
        let snapshot = self
            .route_explain_scheduler
            .as_ref()
            .map_or_else(|| self.registry.load(), |scheduler| scheduler.snapshot());
        (snapshot.version().as_str() == version_id.as_str())
            .then_some(snapshot)
            .ok_or(ManagementRuntimeError::Unavailable)
    }

    /// Recovers one operator-confirmed forbidden account through the controlled ticket flow.
    ///
    /// The operator's authenticated request is the account-level evidence (BL-16/BL-17): a
    /// forbidden binding never reopens from data-plane traffic because selection excludes it.
    /// Begin and complete stay one local transition; no Provider request is sent.
    fn recover_forbidden_account(
        &self,
        target: &ManagementRuntimeTarget,
        observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        let expires_at_ms = observed_at_ms
            .checked_add(P12_OPERATOR_RECOVERY_TTL_MS)
            .ok_or(ManagementRuntimeError::Unavailable)?;
        let Some(ticket) = self
            .runtime_health
            .begin_account_recovery(target.endpoint_id(), target.credential_id(), expires_at_ms)
            .map_err(|_| ManagementRuntimeError::Unavailable)?
        else {
            // A concurrent recovery already owns this binding; its owner reports the outcome.
            return Ok(ManagementQuotaRecoveryState::ProbeScheduled);
        };
        self.runtime_health
            .complete_account_recovery(ticket, RuntimeHealthAccountRecoveryResult::Allowed)
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        Ok(ManagementQuotaRecoveryState::Released)
    }

    /// Completes one due controlled quota recovery as an explicit operator override.
    ///
    /// A pre-Reset exhausted window is refused (`RecoveryRequired`): BL-17 admits a controlled
    /// probe only after Reset, and an operator cannot move an upstream reset window. The live
    /// selection path remains the automatic probe owner; this override exists for a due target
    /// that receives no traffic.
    fn recover_quota_target(
        &self,
        target: &ManagementRuntimeTarget,
        observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        let quota_target = match target.upstream_model() {
            Some(model) => RuntimeQuotaTarget::endpoint_credential_model(
                target.endpoint_id().clone(),
                target.credential_id().clone(),
                model,
            )
            .map_err(|_| ManagementRuntimeError::InvalidInput)?,
            None => RuntimeQuotaTarget::endpoint_credential(
                target.endpoint_id().clone(),
                target.credential_id().clone(),
            ),
        };
        let availability = self
            .runtime_quota
            .availability_at(&quota_target, observed_at_ms)
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        match availability {
            RuntimeQuotaAvailability::Available => Ok(ManagementQuotaRecoveryState::Rejected),
            RuntimeQuotaAvailability::Exhausted { .. } => {
                Ok(ManagementQuotaRecoveryState::RecoveryRequired)
            }
            RuntimeQuotaAvailability::RecoveryProbeInFlight { .. } => {
                Ok(ManagementQuotaRecoveryState::ProbeScheduled)
            }
            RuntimeQuotaAvailability::RecoveryRequired { .. } => {
                self.complete_due_quota_recovery(quota_target, observed_at_ms)
            }
        }
    }

    fn complete_due_quota_recovery(
        &self,
        quota_target: RuntimeQuotaTarget,
        observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        let expires_at_ms = observed_at_ms
            .checked_add(P12_OPERATOR_RECOVERY_TTL_MS)
            .ok_or(ManagementRuntimeError::Unavailable)?;
        let Some(ticket) = self
            .runtime_quota
            .begin_recovery_probe(&quota_target, expires_at_ms)
            .map_err(|_| ManagementRuntimeError::Unavailable)?
        else {
            // The live selection path already owns a probe; it reports the outcome.
            return Ok(ManagementQuotaRecoveryState::ProbeScheduled);
        };
        let snapshot = QuotaSnapshot::try_new(
            quota_target,
            Vec::new(),
            QuotaSource::Estimated,
            QuotaConfidence::Estimated,
            observed_at_ms,
        )
        .map_err(|_| ManagementRuntimeError::Unavailable)?;
        self.runtime_quota
            .complete_recovery_probe(ticket, snapshot)
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        Ok(ManagementQuotaRecoveryState::Released)
    }
}

fn management_route_explain_reason(candidate: &RouteExplainCandidate) -> &'static str {
    candidate
        .reasons()
        .iter()
        .map(|reason| match reason {
            RouteExplainCandidateReason::NotHardEligible => "not_hard_eligible",
            RouteExplainCandidateReason::EndpointHealth(availability) => match availability {
                gateway_router::RuntimeHealthAvailability::CoolingDown { .. } => {
                    "endpoint_cooldown"
                }
                gateway_router::RuntimeHealthAvailability::CircuitOpen { .. } => {
                    "endpoint_circuit_open"
                }
                gateway_router::RuntimeHealthAvailability::AccountForbidden
                | gateway_router::RuntimeHealthAvailability::CredentialUnauthorized
                | gateway_router::RuntimeHealthAvailability::AccountRecoveryInFlight { .. }
                | gateway_router::RuntimeHealthAvailability::Available => "endpoint_unavailable",
            },
            RouteExplainCandidateReason::EndpointHealthUnavailable => "endpoint_unavailable",
            RouteExplainCandidateReason::MissingCredentialPool => "missing_credential_pool",
            RouteExplainCandidateReason::NoEligibleCredential => "no_eligible_credential",
        })
        .next()
        .unwrap_or("no_eligible_credential")
}

const fn management_price_evidence(value: ProviderScopedPriceEvidence) -> &'static str {
    match value {
        ProviderScopedPriceEvidence::Dominant => "dominant",
        ProviderScopedPriceEvidence::Equal => "equal",
        ProviderScopedPriceEvidence::Dominated => "dominated",
        ProviderScopedPriceEvidence::Incomparable => "incomparable",
        ProviderScopedPriceEvidence::Unpriced => "unpriced",
        ProviderScopedPriceEvidence::NotEvaluated => "not_evaluated",
    }
}

const fn management_price_comparison(value: RoutingPriceComparison) -> &'static str {
    match value {
        RoutingPriceComparison::RateDominanceV1 => "rate_dominance_v1",
    }
}

const fn management_catalog_failure_class(
    value: CatalogDiscoveryFailureClass,
) -> ManagementCatalogFailureClass {
    match value {
        CatalogDiscoveryFailureClass::Authentication => {
            ManagementCatalogFailureClass::Authentication
        }
        CatalogDiscoveryFailureClass::Authorization => ManagementCatalogFailureClass::Authorization,
        CatalogDiscoveryFailureClass::RateLimit => ManagementCatalogFailureClass::RateLimit,
        CatalogDiscoveryFailureClass::Transport => ManagementCatalogFailureClass::Transport,
        CatalogDiscoveryFailureClass::Upstream => ManagementCatalogFailureClass::Upstream,
        CatalogDiscoveryFailureClass::Internal => ManagementCatalogFailureClass::Internal,
    }
}

impl ManagementRuntimeFacade for SnapshotManagementRuntimeFacade {
    fn effective_models(
        &mut self,
        config_version_id: &gateway_store::control_plane::ConfigVersionId,
        context: &gateway_http_actix::management_resources::ManagementEffectiveModelContext,
        observed_at_ms: i64,
    ) -> Result<
        Option<gateway_http_actix::management_resources::ManagementEffectiveModels>,
        ManagementRuntimeError,
    > {
        use gateway_http_actix::management_resources::{
            ManagementEffectiveModel, ManagementEffectiveModelContext,
            ManagementEffectiveModelSource, ManagementEffectiveModels,
        };
        use gateway_router::SnapshotCatalogAdmission;
        if observed_at_ms < 0 {
            return Err(ManagementRuntimeError::InvalidInput);
        }
        let snapshot = self.snapshot_for(config_version_id)?;
        let (group, client_key_id) = match context {
            ManagementEffectiveModelContext::AccessGroup(id) => (snapshot.access_group(id), None),
            ManagementEffectiveModelContext::ClientKey(id) => (
                snapshot.access_group_for_client_key_at(id, observed_at_ms),
                Some(id.as_str().to_owned()),
            ),
        };
        let Some(group) = group else {
            return Ok(None);
        };
        let Some(models) =
            snapshot.effective_models_for_access_group_at(group.id(), observed_at_ms)
        else {
            return Ok(None);
        };
        let models = models
            .into_iter()
            .map(|model| ManagementEffectiveModel {
                id: model.exact_id.to_owned(),
                public_model_id: model.public_model.id().as_str().to_owned(),
                public_model_name: model.public_model.model_name().to_owned(),
                route_id: model.public_model.route_id().as_str().to_owned(),
                sources: model
                    .candidates
                    .into_iter()
                    .map(|candidate| ManagementEffectiveModelSource {
                        capability_evidence: self.execution_configuration.as_ref().filter(|(version, _)| version == snapshot.version()).map(|(version, revision)| gateway_http_actix::management_resources::ManagementCapabilityEvidence {
                            source: "serving_snapshot", config_version_id: version.as_str().to_owned(), config_revision: revision.as_i64(), capabilities: execution_identity::effective_capabilities(candidate, self.execution_owned_reasoning.contains(candidate.endpoint_id())),
                        }),
                        candidate_id: candidate.id().as_str().to_owned(),
                        endpoint_id: candidate.endpoint_id().as_str().to_owned(),
                        upstream_id: candidate.upstream_id().as_str().to_owned(),
                        api_format: candidate.endpoint_api_format().to_owned(),
                        catalog_evidence: candidate.catalog_evidence().map(|(id, evidence)|
                            gateway_http_actix::management_resources::ManagementModelCatalogEvidence {
                                credential_id: id.as_str().to_owned(), version: evidence.version,
                                observed_at_ms: evidence.observed_at_ms, stale_at_ms: evidence.stale_at_ms,
                                expires_at_ms: evidence.expires_at_ms,
                                catalog_eligible: candidate.allows_credential_at(id, observed_at_ms),
                            }).collect(),
                        catalog_admission: match candidate.catalog_admission() {
                            SnapshotCatalogAdmission::AllowedUnlisted => "allowed_unlisted",
                            SnapshotCatalogAdmission::Listed(CatalogModelState::Manual) => "manual",
                            SnapshotCatalogAdmission::Listed(CatalogModelState::Fresh) => "fresh",
                            SnapshotCatalogAdmission::Listed(CatalogModelState::Stale) => "stale",
                            SnapshotCatalogAdmission::Listed(CatalogModelState::Expired) => {
                                "expired"
                            }
                        },
                    })
                    .collect(),
            })
            .collect();
        Ok(Some(ManagementEffectiveModels {
            config_version: snapshot.version().as_str().to_owned(),
            access_group_id: group.id().as_str().to_owned(),
            client_key_id,
            models,
        }))
    }

    fn catalog_status(
        &mut self,
        config_version_id: &gateway_store::control_plane::ConfigVersionId,
        observed_at_ms: i64,
    ) -> Result<Vec<ManagementCatalogStatus>, ManagementRuntimeError> {
        self.snapshot_for(config_version_id)?;
        let mut targets = BTreeSet::new();
        let mut statuses = self
            .catalog_store
            .list_statuses_at(config_version_id.as_str(), observed_at_ms)
            .map_err(|_| ManagementRuntimeError::Unavailable)?
            .into_iter()
            .map(|status| {
                let freshness = match status.freshness() {
                    CatalogSnapshotFreshness::Fresh => ManagementCatalogFreshness::Fresh,
                    CatalogSnapshotFreshness::Stale => ManagementCatalogFreshness::Stale,
                    CatalogSnapshotFreshness::Expired => ManagementCatalogFreshness::Expired,
                };
                let failure = status
                    .last_failure()
                    .map(|(at, class)| (at, management_catalog_failure_class(class)));
                targets.insert(status.snapshot().target().clone());
                Ok(ManagementCatalogStatus::new(
                    status.snapshot().target().endpoint_id().clone(),
                    status.snapshot().target().credential_id().clone(),
                    freshness,
                    status.snapshot().observed_at_ms(),
                )
                .with_discovery_details(
                    status.snapshot().version(),
                    status.is_refresh_due(),
                    status
                        .models()
                        .iter()
                        .filter(|model| model.is_present_in_last_success())
                        .count(),
                    failure,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for failure in self
            .catalog_store
            .list_failures(config_version_id.as_str())
            .map_err(|_| ManagementRuntimeError::Unavailable)?
        {
            if targets.contains(failure.target()) {
                continue;
            }
            statuses.push(
                ManagementCatalogStatus::new(
                    failure.target().endpoint_id().clone(),
                    failure.target().credential_id().clone(),
                    ManagementCatalogFreshness::Missing,
                    0,
                )
                .with_last_failure(
                    failure.failed_at_ms(),
                    management_catalog_failure_class(failure.class()),
                ),
            );
        }
        for (target, supported) in &self.catalog_targets {
            if let Some(status) = statuses.iter_mut().find(|status| {
                status.endpoint_id() == target.endpoint_id()
                    && status.credential_id() == target.credential_id()
            }) {
                *status = status.clone().with_discovery_support(*supported);
            } else {
                statuses.push(
                    ManagementCatalogStatus::new(
                        target.endpoint_id().clone(),
                        target.credential_id().clone(),
                        ManagementCatalogFreshness::Missing,
                        0,
                    )
                    .with_discovery_support(*supported),
                );
            }
        }
        statuses.sort_by(|left, right| {
            (left.endpoint_id(), left.credential_id())
                .cmp(&(right.endpoint_id(), right.credential_id()))
        });
        Ok(statuses)
    }

    fn runtime_availability(
        &mut self,
        config_version_id: &gateway_store::control_plane::ConfigVersionId,
        observed_at_ms: i64,
    ) -> Result<Vec<ManagementRuntimeAvailabilityStatus>, ManagementRuntimeError> {
        use gateway_http_actix::management_resources::ManagementRuntimeAvailability as State;
        use gateway_router::{RuntimeHealthAvailability as Health, RuntimeHealthKey};
        if observed_at_ms < 0 {
            return Err(ManagementRuntimeError::InvalidInput);
        }
        self.snapshot_for(config_version_id)?;
        let Some(scheduler) = self.route_explain_scheduler.as_ref() else {
            return Ok(Vec::new());
        };
        let mut rows = Vec::new();
        // Match the HTTP projection bound; never return a silently truncated matrix.
        for (endpoint, credential) in scheduler.diagnostic_bindings().take(257) {
            if rows.len() == 256 {
                return Err(ManagementRuntimeError::Unavailable);
            }
            let id = credential.credential_id();
            let mut states = Vec::with_capacity(4);
            if credential
                .expires_at_ms()
                .is_some_and(|expiry| expiry <= observed_at_ms)
            {
                states.push(State::Expired);
            }
            for key in [
                RuntimeHealthKey::endpoint(endpoint.clone()),
                RuntimeHealthKey::endpoint_credential(endpoint.clone(), id.clone()),
            ] {
                states.push(
                    match self
                        .runtime_health
                        .availability_at(&key, observed_at_ms)
                        .map_err(|_| ManagementRuntimeError::Unavailable)?
                    {
                        Health::Available => State::Available,
                        Health::CoolingDown { .. } => State::Cooldown,
                        Health::CircuitOpen { .. } => State::CircuitOpen,
                        Health::AccountForbidden => State::CredentialForbidden,
                        Health::CredentialUnauthorized => State::CredentialUnauthorized,
                        Health::AccountRecoveryInFlight { .. } => State::RecoveryRequired,
                    },
                );
            }
            states.push(
                match self
                    .runtime_quota
                    .availability_at(
                        &RuntimeQuotaTarget::endpoint_credential(endpoint.clone(), id.clone()),
                        observed_at_ms,
                    )
                    .map_err(|_| ManagementRuntimeError::Unavailable)?
                {
                    RuntimeQuotaAvailability::Available => State::Available,
                    RuntimeQuotaAvailability::Exhausted { .. } => State::QuotaBlocked,
                    RuntimeQuotaAvailability::RecoveryRequired { .. }
                    | RuntimeQuotaAvailability::RecoveryProbeInFlight { .. } => {
                        State::RecoveryRequired
                    }
                },
            );
            // Stronger persistent account restrictions win over transient endpoint signals.
            let availability = states
                .into_iter()
                .max_by_key(|state| match state {
                    State::Expired => 7,
                    State::CredentialUnauthorized => 6,
                    State::CredentialForbidden => 5,
                    State::RecoveryRequired => 4,
                    State::QuotaBlocked => 3,
                    State::CircuitOpen => 2,
                    State::Cooldown => 1,
                    State::Available => 0,
                })
                .unwrap_or(State::Available);
            rows.push(ManagementRuntimeAvailabilityStatus::new(
                endpoint,
                id.clone(),
                availability,
            ));
        }
        Ok(rows)
    }

    fn request_quota_recovery(
        &mut self,
        config_version_id: &gateway_store::control_plane::ConfigVersionId,
        target: &ManagementRuntimeTarget,
        observed_at_ms: i64,
    ) -> Result<ManagementQuotaRecoveryState, ManagementRuntimeError> {
        self.snapshot_for(config_version_id)?;
        let account_status = self
            .runtime_health
            .credential_account_status_at(
                target.endpoint_id(),
                target.credential_id(),
                observed_at_ms,
            )
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        match account_status {
            RuntimeCredentialAccountStatus::Unauthorized => {
                Ok(ManagementQuotaRecoveryState::Rejected)
            }
            RuntimeCredentialAccountStatus::Forbidden => {
                // An account block covers the whole binding, so only a binding-scoped request may
                // clear it. A model-scoped target expresses quota intent; honouring it here would
                // let a request addressing one model lift an account-level block it never named.
                if target.upstream_model().is_some() {
                    return Ok(ManagementQuotaRecoveryState::Rejected);
                }
                let quota = self.recover_quota_target(target, observed_at_ms)?;
                if matches!(
                    quota,
                    ManagementQuotaRecoveryState::RecoveryRequired
                        | ManagementQuotaRecoveryState::ProbeScheduled
                ) {
                    return Ok(quota);
                }
                self.recover_forbidden_account(target, observed_at_ms)
            }
            RuntimeCredentialAccountStatus::RecoveryInFlight { .. } => {
                Ok(ManagementQuotaRecoveryState::ProbeScheduled)
            }
            RuntimeCredentialAccountStatus::Available => {
                self.recover_quota_target(target, observed_at_ms)
            }
        }
    }

    #[allow(clippy::too_many_lines)] // Keep the value-free scope/admission/reason projection together.
    fn explain_route(
        &mut self,
        request: &ManagementRouteExplainRequest,
    ) -> Result<ManagementRouteExplain, ManagementRuntimeError> {
        let snapshot = self.snapshot_for(request.config_version_id())?;
        if self
            .route_explain_scheduler
            .as_ref()
            .is_some_and(|scheduler| {
                scheduler.snapshot_version().as_str() != request.config_version_id().as_str()
            })
            || self.routing_price_snapshot.as_ref().is_some_and(|price| {
                price.config_version_id() != request.config_version_id()
                    || price.snapshot_version().as_str() != request.config_version_id().as_str()
            })
        {
            return Err(ManagementRuntimeError::Unavailable);
        }
        let mut name = request.requested_model();
        let mut name_thinking = None;
        if !snapshot.has_exact_model_name(name)
            && let Some((base, thinking)) = gateway_router::parse_thinking_suffix(name)
                .map_err(|_| ManagementRuntimeError::InvalidInput)?
        {
            name = base;
            name_thinking = Some(thinking);
        }
        let (public_model, exact_upstream_model) = match snapshot
            .resolve_exact_upstream_model_for_management_at(name, request.observed_at_ms())
        {
            gateway_router::SnapshotExactModelResolution::Unique(model) => (model, Some(name)),
            gateway_router::SnapshotExactModelResolution::Ambiguous => {
                return Err(ManagementRuntimeError::Unavailable);
            }
            gateway_router::SnapshotExactModelResolution::Absent => (
                snapshot
                    .resolve_public_model(name)
                    .ok_or(ManagementRuntimeError::Unavailable)?,
                None,
            ),
        };
        if public_model.route_id() != request.route_id() {
            return Err(ManagementRuntimeError::Unavailable);
        }
        let route = snapshot
            .route(public_model.route_id())
            .filter(|route| route.id() == request.route_id())
            .ok_or(ManagementRuntimeError::Unavailable)?;
        let source = match request.protocol() {
            ManagementRequestProtocol::OpenAiChatCompletions => {
                ProtocolFormat::OpenAiChatCompletions
            }
            ManagementRequestProtocol::OpenAiResponses => ProtocolFormat::OpenAiResponses,
            ManagementRequestProtocol::AnthropicMessages => ProtocolFormat::AnthropicMessages,
        };
        let pair_is_publishable = |candidate: &SnapshotRouteCandidate| {
            candidate.protocol_format().is_some_and(|target| {
                protocol_pair_is_publishable(
                    source,
                    target,
                    candidate.transform_mode(),
                    candidate.effective_capabilities(),
                ) && exact_upstream_model.is_none_or(|model| candidate.upstream_model() == model)
                    && gateway_router::validate_model_name_thinking(
                        name_thinking.as_ref(),
                        source,
                        target,
                        candidate.upstream_model(),
                        candidate.transform_mode(),
                        candidate.effective_capabilities(),
                    )
                    .is_ok()
            })
        };
        let admitted_candidate_ids = route
            .candidates()
            .iter()
            .filter(|candidate| candidate.is_hard_eligible() && pair_is_publishable(candidate))
            .map(|candidate| candidate.id().clone())
            .collect::<BTreeSet<_>>();
        let provider_ids = admitted_candidate_ids
            .iter()
            .filter_map(|candidate_id| {
                route
                    .candidates()
                    .iter()
                    .find(|candidate| candidate.id() == candidate_id)
                    .map(|candidate| candidate.upstream_id().clone())
            })
            .map(|upstream_id| {
                ProviderId::try_new(upstream_id.as_str().to_owned())
                    .map_err(|_| ManagementRuntimeError::Unavailable)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let exact_model_sources = exact_upstream_model.is_some()
            || route
                .candidates()
                .iter()
                .all(|candidate| candidate.upstream_model() == public_model.model_name());
        let inferred_provider = match request.provider_id().cloned() {
            Some(provider_id) => Some(provider_id),
            None if provider_ids.len() == 1 => provider_ids.iter().next().cloned(),
            None => None,
        };

        let projected = if let Some(scheduler) = &self.route_explain_scheduler {
            let route_input = RouteExplainInput::new(route.id().clone(), request.observed_at_ms());
            if let Some(provider_id) = inferred_provider.clone() {
                let composition_input = ProviderScopedRouteExplainInput::try_new(
                    route_input,
                    provider_id,
                    admitted_candidate_ids.clone(),
                    BTreeMap::new(),
                )
                .map_err(|_| ManagementRuntimeError::Unavailable)?;
                Some(
                    scheduler
                        .explain_provider_scoped(
                            &composition_input,
                            &self.runtime_health,
                            &self.runtime_quota,
                            &AttemptExclusionSet::new(),
                        )
                        .map_err(|_| ManagementRuntimeError::Unavailable)?,
                )
            } else {
                None
            }
        } else {
            None
        };
        let base_explain = if projected.is_none() {
            self.route_explain_scheduler
                .as_ref()
                .map(|scheduler| {
                    scheduler
                        .explain_admitted(
                            &RouteExplainInput::new(route.id().clone(), request.observed_at_ms()),
                            &self.runtime_health,
                            &self.runtime_quota,
                            &AttemptExclusionSet::new(),
                            &admitted_candidate_ids,
                        )
                        .map_err(|_| ManagementRuntimeError::Unavailable)
                })
                .transpose()?
        } else {
            None
        };
        let selected = projected
            .as_ref()
            .and_then(|value| value.provider_selection().selected_candidate_id());
        let price_evidence_by_candidate = self
            .routing_price_snapshot
            .as_ref()
            .and(projected.as_ref())
            .map(|value| {
                value
                    .provider_selection()
                    .decisions()
                    .iter()
                    .map(|decision| {
                        (
                            decision.candidate().candidate_id().clone(),
                            management_price_evidence(decision.price_evidence()),
                        )
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let default_price_evidence = if self.routing_price_snapshot.is_some() {
            "not_evaluated"
        } else {
            "disabled"
        };
        let ambiguous_provider_scope = self.route_explain_scheduler.is_some()
            && request.provider_id().is_none()
            && provider_ids.len() > 1
            && !exact_model_sources;
        let legacy_selected = if projected.is_none()
            && !ambiguous_provider_scope
            && request.provider_id().is_none()
        {
            if let Some(base) = &base_explain {
                base.projected_selection()
                    .map(|selection| selection.candidate_id().clone())
            } else {
                route
                    .candidates()
                    .iter()
                    .find(|candidate| {
                        candidate.is_hard_eligible() && pair_is_publishable(candidate)
                    })
                    .map(|candidate| candidate.id().clone())
            }
        } else {
            None
        };
        let candidates = route
            .candidates()
            .iter()
            .map(|candidate| {
                let pair_is_publishable = pair_is_publishable(candidate);
                let base_candidate = projected
                    .as_ref()
                    .map(ProviderScopedRouteExplainSnapshot::base)
                    .or(base_explain.as_ref())
                    .and_then(|snapshot| {
                        snapshot
                            .candidates()
                            .iter()
                            .find(|value| value.candidate_id() == candidate.id())
                    });
                let price_evidence = price_evidence_by_candidate
                    .get(candidate.id())
                    .copied()
                    .unwrap_or(default_price_evidence);
                let explain = if selected.is_some_and(|value| value == candidate.id())
                    || legacy_selected.as_ref() == Some(candidate.id())
                {
                    ManagementRouteExplainCandidate::selected(candidate.id().clone())
                } else if !candidate.is_hard_eligible() {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        "not_hard_eligible",
                    )
                } else if !pair_is_publishable {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        "protocol_transform_unavailable",
                    )
                } else if let Some(base_candidate) =
                    base_candidate.filter(|value| !value.is_eligible())
                {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        management_route_explain_reason(base_candidate),
                    )
                } else if ambiguous_provider_scope {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        "provider_scope_required",
                    )
                } else if request.provider_id().is_some_and(|provider_id| {
                    ProviderId::try_new(candidate.upstream_id().as_str().to_owned())
                        .is_ok_and(|candidate_provider| candidate_provider != *provider_id)
                }) {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        "provider_mismatch",
                    )
                } else {
                    ManagementRouteExplainCandidate::excluded(
                        candidate.id().clone(),
                        "after_selected_candidate",
                    )
                };
                explain.with_price_evidence(price_evidence)
            })
            .collect();
        let explain = ManagementRouteExplain::try_new(route.id().clone(), candidates)?
            .with_selection_policy(route.policy().serving_strategy());
        Ok(match &self.routing_price_snapshot {
            Some(price_snapshot) => {
                let policy = ManagementRouteExplainPricePolicy::new(
                    price_snapshot.catalog_version_id().to_owned(),
                    management_price_comparison(price_snapshot.comparison()),
                )?;
                explain.with_price_policy(policy)
            }
            None => explain,
        })
    }

    fn list_request_attempts(
        &mut self,
        request_id: &gateway_core::RequestId,
    ) -> Result<Vec<ManagementRequestAttempt>, ManagementRuntimeError> {
        let events = self
            .event_store
            .events_for_request(request_id)
            .map_err(|_| ManagementRuntimeError::Unavailable)?;
        let mut attempts = Vec::new();
        for stored in &events {
            let GatewayEvent::Attempt(attempt) = stored.event() else {
                continue;
            };
            let outcome = match attempt.outcome() {
                AttemptOutcome::Succeeded => "succeeded",
                AttemptOutcome::Failed(_) => "failed",
            };
            attempts.push(
                ManagementRequestAttempt::try_new(
                    attempt.attempt_id().as_str().to_owned(),
                    outcome,
                    Some(attempt.endpoint_id().clone()),
                    Some(attempt.credential_id().clone()),
                )?
                .with_observation(attempt),
            );
        }
        // The in-process ledger records per-attempt terminals and one request-level stage that
        // describes the newest attempt. It is enrichment, but it is also the only evidence that
        // an attempt this process observed is missing from the durable log: a terminal the
        // bounded queue rejected never becomes durable, so serving the shorter durable list as
        // success would report a failed attempt as never having happened. Fail closed instead,
        // and pair the stage only onto an exactly matching single-attempt timeline, where the
        // request-level stage provably describes that attempt and no in-flight successor.
        if let Ok(recorded) = self.attempt_stages.list_request_attempts(request_id) {
            if recorded.len() > attempts.len() {
                return Err(ManagementRuntimeError::Unavailable);
            }
            if recorded.len() == attempts.len()
                && recorded.len() == 1
                && recorded
                    .iter()
                    .zip(&attempts)
                    .all(|(ledger, stored)| ledger.attempt_id() == stored.attempt_id())
                && let Some(stage) = recorded.last().and_then(ManagementRequestAttempt::stage)
                && let Some(attempt) = attempts.pop()
            {
                attempts.push(attempt.with_stage(stage));
            }
        }
        Ok(attempts)
    }
}

#[cfg(test)]
mod model_name_tests {
    use super::*;
    use gateway_catalog::{CapabilitySet, SemanticCapability};
    use gateway_core::{PublicModelId, RouteCandidateId, RouteId, UpstreamId};
    use gateway_router::{
        RouteSnapshotInput, SnapshotCatalogAdmission, SnapshotPublicModel, SnapshotRoute,
        SnapshotRouteCandidateInput, SnapshotRoutePolicy, SnapshotTransformMode,
    };

    #[test]
    fn cpa_69_explain_prefers_exact_id_and_constrains_candidates()
    -> Result<(), Box<dyn std::error::Error>> {
        let version = gateway_store::control_plane::ConfigVersionId::try_new("name-explain")?;
        let mut models = Vec::new();
        let mut routes = Vec::new();
        let mut ids = Vec::new();
        for index in ["a", "b"] {
            let id = PublicModelId::try_new(format!("model-{index}"))?;
            let route = RouteId::try_new(format!("route-{index}"))?;
            models.push(SnapshotPublicModel::new(
                id.clone(),
                format!("public-{index}"),
                index.into(),
                CapabilitySet::empty(),
                route.clone(),
            ));
            let candidate = |suffix: &str, model: &str| -> Result<_, Box<dyn std::error::Error>> {
                Ok(SnapshotRouteCandidate::new(SnapshotRouteCandidateInput {
                    id: RouteCandidateId::try_new(format!("candidate-{index}-{suffix}"))?,
                    endpoint_id: EndpointId::try_new(format!("endpoint-{index}"))?,
                    upstream_id: UpstreamId::try_new(format!("upstream-{index}"))?,
                    endpoint_api_format: "openai/responses".into(),
                    upstream_model: model.into(),
                    transform_mode: SnapshotTransformMode::CanonicalBridge,
                    priority: 0,
                    weight: 1,
                    effective_capabilities: CapabilitySet::try_new([
                        SemanticCapability::Reasoning,
                    ])?,
                    catalog_admission: SnapshotCatalogAdmission::AllowedUnlisted,
                    active_binding_count: 1,
                }))
            };
            let candidates = if index == "a" {
                vec![
                    candidate("exact", "team/model")?,
                    candidate("other", "other-model")?,
                ]
            } else {
                vec![candidate("alias", "alias-upstream")?]
            };
            routes.push(SnapshotRoute::new(
                route,
                id.clone(),
                SnapshotRoutePolicy::RoundRobin,
                1,
                1000,
                candidates,
            ));
            ids.push(id);
        }
        let snapshot = Arc::new(RouteSnapshot::try_new(RouteSnapshotInput::new(
            SnapshotVersion::try_new(version.as_str())?,
            models,
            vec![("team/model".into(), ids[1].clone())],
            routes,
            Vec::new(),
            Vec::new(),
        ))?);
        let mut facade = SnapshotManagementRuntimeFacade {
            execution_configuration: None,
            execution_owned_reasoning: BTreeSet::new(),
            registry: Arc::new(RouteSnapshotRegistry::new(snapshot)),
            attempt_stages: Arc::new(P12AttemptStageStore::new()),
            runtime_health: Arc::new(RuntimeHealthRegistry::new()),
            runtime_quota: Arc::new(RuntimeQuotaRegistry::new()),
            route_explain_scheduler: None,
            routing_price_snapshot: None,
            event_store: SqliteEventStore::open_in_memory()?,
            catalog_store: SqliteCatalogSnapshotStore::open_in_memory()?,
            catalog_targets: Vec::new(),
        };
        for name in ["team/model", "team/model(high)"] {
            let request = |route: &str| {
                ManagementRouteExplainRequest::try_new(
                    version.clone(),
                    RouteId::try_new(route).map_err(|_| ManagementRuntimeError::InvalidInput)?,
                    name.into(),
                    ManagementRequestProtocol::OpenAiResponses,
                    None,
                    1,
                )
            };
            assert!(
                facade
                    .explain_route(&request("route-b").map_err(|_| "request")?)
                    .is_err()
            );
            let result = facade
                .explain_route(&request("route-a").map_err(|_| "request")?)
                .map_err(|_| "explain")?;
            assert!(result.candidates()[0].selected_by_projection());
            assert!(!result.candidates()[1].selected_by_projection());
        }
        Ok(())
    }
}
