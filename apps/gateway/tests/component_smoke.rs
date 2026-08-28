//! P0 smoke test proving that the process crate links only its intended top-level boundaries.

#![deny(unsafe_code)]

#[test]
fn application_links_expected_top_level_components() {
    let components = [
        gateway_control::COMPONENT,
        gateway_http_actix::COMPONENT,
        gateway_observability::COMPONENT,
    ];

    assert_eq!(
        components,
        [
            "gateway-control",
            "gateway-http-actix",
            "gateway-observability",
        ]
    );
}
