CREATE INDEX gateway_request_client_key ON gateway_event_log(json_extract(payload_json,'$.request.client_key_id'), request_id) WHERE event_type='request' AND json_valid(payload_json);
