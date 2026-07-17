//! Process entry point. Runtime bootstrap is implemented in P1 and later phases.

#![deny(unsafe_code)]

fn main() {
    let components = [
        gateway_control::COMPONENT,
        gateway_http_actix::COMPONENT,
        gateway_observability::COMPONENT,
    ];
    println!("gateway skeleton: {} components linked", components.len());
}
