//! P7-08 Kiro network/account/model/quota/rate-limit classification matrix.

use provider_kiro::failure_classification::{
    KiroFailureAction, KiroFailureSignal, KiroNetworkFailure, classify_kiro_http_failure,
    classify_kiro_network_failure,
};

use gateway_core::{ErrorScope, GatewayErrorCode};

#[test]
fn network_failures_are_egress_local_until_semantic_stream_delivery() {
    for failure in [
        KiroNetworkFailure::Dns,
        KiroNetworkFailure::Connect,
        KiroNetworkFailure::TlsHandshake,
        KiroNetworkFailure::FirstByteTimeout,
    ] {
        let disposition = classify_kiro_network_failure(failure);
        assert_eq!(
            disposition.error().code(),
            GatewayErrorCode::EgressUnavailable
        );
        assert_eq!(disposition.error().scope(), ErrorScope::Egress);
        assert_eq!(disposition.action(), KiroFailureAction::RebuildEgress);
    }

    let stream = classify_kiro_network_failure(KiroNetworkFailure::StreamInterrupted);
    assert_eq!(stream.error().code(), GatewayErrorCode::StreamTruncated);
    assert_eq!(stream.error().scope(), ErrorScope::Stream);
    assert_eq!(stream.action(), KiroFailureAction::TerminateStream);
}

#[test]
fn http_signal_matrix_keeps_account_model_quota_and_ordinary_429_distinct() {
    assert_disposition(
        &classify_kiro_http_failure(401, KiroFailureSignal::None),
        GatewayErrorCode::CredentialUnauthorized,
        ErrorScope::Credential,
        KiroFailureAction::RequireReauthorization,
    );
    assert_disposition(
        &classify_kiro_http_failure(403, KiroFailureSignal::None),
        GatewayErrorCode::EgressRejected,
        ErrorScope::Egress,
        KiroFailureAction::None,
    );
    assert_disposition(
        &classify_kiro_http_failure(403, KiroFailureSignal::AccountForbidden),
        GatewayErrorCode::CredentialForbidden,
        ErrorScope::Account,
        KiroFailureAction::MarkAccountForbidden,
    );
    assert_disposition(
        &classify_kiro_http_failure(404, KiroFailureSignal::ModelUnavailable),
        GatewayErrorCode::RouteNotFound,
        ErrorScope::Model,
        KiroFailureAction::CoolModel,
    );
    assert_disposition(
        &classify_kiro_http_failure(429, KiroFailureSignal::QuotaExhausted),
        GatewayErrorCode::CredentialQuotaExceeded,
        ErrorScope::QuotaWindow,
        KiroFailureAction::CoolQuotaWindow,
    );
    assert_disposition(
        &classify_kiro_http_failure(429, KiroFailureSignal::AccountRateLimited),
        GatewayErrorCode::ProviderRateLimited,
        ErrorScope::Account,
        KiroFailureAction::CoolAccount,
    );
    assert_disposition(
        &classify_kiro_http_failure(429, KiroFailureSignal::None),
        GatewayErrorCode::ProviderRateLimited,
        ErrorScope::Provider,
        KiroFailureAction::CoolProvider,
    );
    assert_disposition(
        &classify_kiro_http_failure(503, KiroFailureSignal::None),
        GatewayErrorCode::ProviderTransient,
        ErrorScope::Provider,
        KiroFailureAction::CoolProvider,
    );
}

#[test]
fn independently_known_signal_has_precedence_without_retaining_a_body() {
    assert_disposition(
        &classify_kiro_http_failure(500, KiroFailureSignal::ModelUnavailable),
        GatewayErrorCode::RouteNotFound,
        ErrorScope::Model,
        KiroFailureAction::CoolModel,
    );
    assert_disposition(
        &classify_kiro_http_failure(400, KiroFailureSignal::CredentialUnauthorized),
        GatewayErrorCode::CredentialUnauthorized,
        ErrorScope::Credential,
        KiroFailureAction::RequireReauthorization,
    );
    let diagnostic = format!(
        "{:?}{:?}",
        classify_kiro_http_failure(429, KiroFailureSignal::QuotaExhausted),
        KiroFailureSignal::AccountForbidden,
    );
    for forbidden in ["token", "response", "header", "body", "secret"] {
        assert!(!diagnostic.contains(forbidden));
    }
}

fn assert_disposition(
    disposition: &provider_kiro::failure_classification::KiroFailureDisposition,
    code: GatewayErrorCode,
    scope: ErrorScope,
    action: KiroFailureAction,
) {
    assert_eq!(disposition.error().code(), code);
    assert_eq!(disposition.error().scope(), scope);
    assert_eq!(disposition.action(), action);
}
