//! Bounded execution facts captured from a pinned configuration and an actual lease.

use serde::{Deserialize, Serialize};

/// Complete gateway declaration for one compiled candidate, before protocol applicability.
/// Missing observations are represented by `None`, never by an all-false declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent compiled capability declarations form a fixed evidence contract"
)]
pub struct EffectiveCapabilities {
    /// Explicit reasoning declaration.
    pub reasoning: bool,
    /// Parallel tool declaration.
    pub parallel: bool,
    /// Gateway-owned stored Responses declaration.
    pub stored: bool,
    /// At least one exact continuation kind is declared.
    pub continuation: bool,
    /// Gateway-owned compaction declaration.
    pub compact: bool,
    /// Downstream Responses WebSocket declaration.
    pub websocket: bool,
}

/// Actual selected transport identity. Direct has no independently revisioned egress resource.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionEgress {
    /// The transport decision could not be observed.
    Unknown,
    /// The driver ended before selecting a transport.
    NotSelected,
    /// Actual direct transport; resource revision is not applicable.
    Direct,
    /// A process-configured proxy whose revision evidence is the actual transport fingerprint,
    /// separate from configuration and independently managed resource revisions.
    ProcessProxy {
        /// SHA256 of the validated secret-free SOCKS5 transport identity.
        transport_fingerprint: String,
    },
    /// A selected version-owned proxy resource. Its revision scope is the configuration.
    ConfiguredProxy {
        /// Exact configured target identity (fixed profile or pool).
        target_id: String,
        /// Exact selected profile/node identity.
        node_id: String,
        /// Version that owns the immutable proxy decision.
        config_version_id: String,
        /// Actual version revision; not a fabricated per-node revision.
        config_revision: i64,
    },
}

/// Actual driver facts, durably bound to the original Attempt payload in a separate table.
/// This does not contain management-account or native-account revisions.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptExecutionIdentity {
    /// Actual adapter/credential channel, independent of endpoint identity.
    pub channel: String,
    /// Revision of the material held by the actual credential lease.
    pub credential_revision: u64,
    /// Actual loaded configuration version ID.
    pub config_version_id: String,
    /// Actual loaded graph revision.
    pub config_revision: i64,
    /// Actual transport selection, including explicit unknown/not-applicable states.
    pub egress: ExecutionEgress,
    /// Complete declaration retained by the selected compiled candidate.
    pub capabilities: EffectiveCapabilities,
}

impl AttemptExecutionIdentity {
    /// Checks the bounded observation contract at persistence and protected-read boundaries.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let identifier = |value: &str| !value.is_empty() && value.len() <= 512;
        let egress_valid = match &self.egress {
            ExecutionEgress::ConfiguredProxy {
                target_id,
                node_id,
                config_version_id,
                config_revision,
            } => {
                identifier(target_id)
                    && identifier(node_id)
                    && config_version_id == &self.config_version_id
                    && config_revision == &self.config_revision
            }
            ExecutionEgress::ProcessProxy {
                transport_fingerprint,
            } => {
                transport_fingerprint.len() == 64
                    && transport_fingerprint
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            }
            _ => true,
        };
        matches!(
            self.channel.as_str(),
            "openai-compatible"
                | "anthropic-compatible"
                | "codex"
                | "claude"
                | "grok.build"
                | "grok.console"
                | "grok.web"
                | "grok.official"
                | "kimi-coding"
                | "kimi-api"
                | "kiro"
        ) && identifier(&self.config_version_id)
            && self.config_revision >= 0
            && egress_valid
            && (!self.capabilities.compact || self.capabilities.stored)
    }
}

impl std::fmt::Debug for AttemptExecutionIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AttemptExecutionIdentity(<protected identifiers>)")
    }
}
