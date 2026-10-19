//! ADR-0114: the qualified Build/CanonicalBridge/Messages delivery exception.
//! This consumes only reviewed summary events; the ordinary projector remains strict.
use gateway_core::{
    CanonicalEvent, CanonicalEventState, GatewayError, MessagesSummaryCompleteness,
    MessagesSummaryEvidence, MessagesSummaryItem, MessagesSummarySource, RawJson, ResponseId,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) struct BuildMessagesSummary {
    source: MessagesSummarySource,
    lifecycle: CanonicalEventState,
    response: Option<ResponseId>,
    next_index: usize,
    reasoning: BTreeMap<String, usize>,
    deltas: BTreeMap<(String, usize), String>,
    confirmed: BTreeMap<usize, RawJson>,
    evidence: Option<MessagesSummaryEvidence>,
    bytes: usize,
}
fn rejected() -> GatewayError {
    GatewayError::response_projection(
        gateway_router::ProtocolResponseRejection::ReasoningUnsupported,
    )
}
impl BuildMessagesSummary {
    pub(super) fn new(streaming: bool) -> Self {
        Self {
            source: if streaming {
                MessagesSummarySource::BuildSse
            } else {
                MessagesSummarySource::BuildJson
            },
            lifecycle: CanonicalEventState::default(),
            response: None,
            next_index: 0,
            reasoning: BTreeMap::new(),
            deltas: BTreeMap::new(),
            confirmed: BTreeMap::new(),
            evidence: None,
            bytes: 0,
        }
    }
    pub(super) fn evidence(&self) -> Option<MessagesSummaryEvidence> {
        self.evidence.clone()
    }
    /// Removes only qualified summary from the internal projection input. Native item identity
    /// events still reach the strict projector's lifecycle; a body-only item keeps its old path.
    pub(super) fn project(
        &mut self,
        event: &CanonicalEvent,
    ) -> Result<Option<CanonicalEvent>, GatewayError> {
        self.lifecycle.apply(event)?;
        match event {
            CanonicalEvent::ResponseStart(start) => self.response = Some(start.response_id.clone()),
            CanonicalEvent::OutputItemStart(item) | CanonicalEvent::OutputItemEnd(item) => {
                let raw = item
                    .extensions
                    .get("openai.responses.output_item")
                    .ok_or_else(rejected)?;
                let mut value: Value = serde_json::from_str(raw.get()).map_err(|_| rejected())?;
                let start = matches!(event, CanonicalEvent::OutputItemStart(_));
                let index = self.next_index;
                if start {
                    self.next_index += 1;
                }
                if value["type"] != "reasoning" {
                    return Ok(Some(event.clone()));
                }
                if item.extensions.iter().len() != 1 || value["id"] != item.item_id {
                    return Err(rejected());
                }
                if start {
                    self.reasoning.insert(item.item_id.clone(), index);
                } else {
                    if value
                        .get("content")
                        .and_then(Value::as_array)
                        .is_some_and(|parts| !parts.is_empty())
                    {
                        // No body obtains omission rights. A body-only item is still judged by
                        // the original strict projector; mixed summary/body remains unsupported.
                        return Ok(Some(event.clone()));
                    }
                    let index = *self.reasoning.get(&item.item_id).ok_or_else(rejected)?;
                    let entry = MessagesSummaryItem {
                        output_index: index,
                        item: raw.clone(),
                    };
                    MessagesSummaryEvidence::try_new(
                        self.source,
                        self.response.clone().ok_or_else(rejected)?,
                        MessagesSummaryCompleteness::ConfirmedItems,
                        super::system_now_ms_runtime().map_err(|_| super::internal_error())?,
                        vec![entry],
                    )?;
                    let parts = value["summary"].as_array().ok_or_else(rejected)?;
                    for ((id, part), text) in &self.deltas {
                        if id == &item.item_id
                            && parts.get(*part).and_then(|v| v["text"].as_str())
                                != Some(text.as_str())
                        {
                            return Err(rejected());
                        }
                    }
                    self.confirmed.insert(index, raw.clone());
                    self.snapshot(MessagesSummaryCompleteness::ConfirmedItems)?;
                    value["summary"] = serde_json::json!([]);
                    let mut projected = item.clone();
                    projected.extensions = gateway_core::RawExtensions::default();
                    projected
                        .extensions
                        .try_insert(
                            "openai.responses.output_item",
                            RawJson::from_json_string(value.to_string()).map_err(|_| rejected())?,
                        )
                        .map_err(|_| rejected())?;
                    return Ok(Some(CanonicalEvent::OutputItemEnd(projected)));
                }
                return Ok(Some(event.clone()));
            }
            CanonicalEvent::ReasoningDelta(delta) => {
                if self.retain_summary_delta(delta)? {
                    return Ok(None);
                }
            }
            CanonicalEvent::ResponseEnd(_) => {
                self.snapshot(MessagesSummaryCompleteness::CompletedResponse)?;
            }
            _ => {}
        }
        Ok(Some(event.clone()))
    }
    fn retain_summary_delta(
        &mut self,
        delta: &gateway_core::ReasoningDelta,
    ) -> Result<bool, GatewayError> {
        let raw = delta
            .extensions
            .get("openai.responses.output_part")
            .ok_or_else(rejected)?;
        let part: Value = serde_json::from_str(raw.get()).map_err(|_| rejected())?;
        if part["field"] != "summary" {
            return Ok(false);
        }
        let id = part["id"].as_str().ok_or_else(rejected)?;
        let index = part["index"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(rejected)?;
        if delta.extensions.iter().len() != 1
            || part.as_object().is_none_or(|v| v.len() != 3)
            || index >= 64
            || !self.reasoning.contains_key(id)
        {
            return Err(rejected());
        }
        self.bytes = self
            .bytes
            .checked_add(delta.text.len())
            .ok_or_else(rejected)?;
        if self.bytes > gateway_core::MAX_MESSAGES_SUMMARY_BYTES {
            return Err(rejected());
        }
        self.deltas
            .entry((id.into(), index))
            .or_default()
            .push_str(&delta.text);
        Ok(true)
    }
    fn snapshot(&mut self, completeness: MessagesSummaryCompleteness) -> Result<(), GatewayError> {
        if !self.confirmed.is_empty() {
            self.evidence = Some(MessagesSummaryEvidence::try_new(
                self.source,
                self.response.clone().ok_or_else(rejected)?,
                completeness,
                super::system_now_ms_runtime().map_err(|_| super::internal_error())?,
                self.confirmed
                    .iter()
                    .map(|(index, item)| MessagesSummaryItem {
                        output_index: *index,
                        item: item.clone(),
                    })
                    .collect(),
            )?);
        }
        Ok(())
    }
}
