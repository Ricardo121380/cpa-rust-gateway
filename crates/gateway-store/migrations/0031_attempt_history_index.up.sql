CREATE INDEX gateway_attempt_request_snapshot
    ON gateway_event_log(request_id,event_ordinal)
    WHERE event_type='attempt';

CREATE INDEX gateway_terminal_history_cover
    ON gateway_event_log(occurred_at_ms,event_ordinal,request_id,
        json_extract(payload_json,'$.request_finished.outcome'),
        json_extract(payload_json,'$.request_finished.duration_ms'),
        json_extract(payload_json,'$.request_finished.first_content_ms'),
        json_extract(payload_json,'$.request_finished.finished_at_ms'))
    WHERE event_type='request_finished';
