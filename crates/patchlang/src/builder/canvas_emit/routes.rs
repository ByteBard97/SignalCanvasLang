use crate::ast::{
    BusEntry, BusOutput, IndexElement, IndexSpec, PortRef, RouteEntry,
};
use crate::builder::canvas_input::*;
use super::ports::*;
use super::helpers::*;

// ---------------------------------------------------------------------------
// Instance routes + buses
// ---------------------------------------------------------------------------

pub(super) fn build_instance_routes(
    routes: &[RouteRuleEmitInput],
    ifaces: &[InterfaceEmitInput],
    installed_cards: &[InstalledCardEmitInput],
    manufacturer_cards: &[CardEmitInput],
    all_instances: &[InstanceEmitInput],
) -> Vec<RouteEntry> {
    routes
        .iter()
        .filter_map(|r| {
            // Drop RF sentinel ports (__rf_receive__, __rf_transmit__) — these are virtual
            // ports used by old frontend versions to represent wireless signal paths. They
            // have no physical port in the template. Design decision: RF routing is out of
            // scope; the emitter drops these rather than emitting unresolvable route refs.
            if is_rf_sentinel(&r.from_interface) || is_rf_sentinel(&r.to_interface) {
                return None;
            }
            let source = resolve_route_endpoint(
                &r.from_interface,
                r.from_channel,
                r.from_instance.as_deref(),
                ifaces,
                installed_cards,
                manufacturer_cards,
                all_instances,
                PortSide::Input,
            );
            let target = resolve_route_endpoint(
                &r.to_interface,
                r.to_channel,
                r.to_instance.as_deref(),
                ifaces,
                installed_cards,
                manufacturer_cards,
                all_instances,
                PortSide::Output,
            );
            Some(RouteEntry {
                source,
                target,
                span: builder_span(),
            })
        })
        .collect()
}

/// Resolve one route endpoint into a `PortRef`, honouring an optional
/// cross-instance qualifier (Backbone-paired Engine↔Surface routing).
///
/// When `instance_qualifier` is `Some`, the port name is resolved against that
/// paired instance's own interfaces/cards (its port names derive from its own
/// template), and the qualifier is carried onto the `PortRef` so the printer
/// emits `PairedInstance.Port[ch]`. When `None`, resolution falls back to the
/// owning instance, matching the original same-instance behaviour.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_route_endpoint(
    interface_id: &str,
    channel: u32,
    instance_qualifier: Option<&str>,
    owning_ifaces: &[InterfaceEmitInput],
    owning_cards: &[InstalledCardEmitInput],
    manufacturer_cards: &[CardEmitInput],
    all_instances: &[InstanceEmitInput],
    side: PortSide,
) -> PortRef {
    let (ifaces, cards): (&[InterfaceEmitInput], &[InstalledCardEmitInput]) = match instance_qualifier
    {
        Some(name) => all_instances
            .iter()
            .find(|i| i.name == name)
            .map(|i| (i.interfaces.as_slice(), i.installed_cards.as_slice()))
            .unwrap_or((owning_ifaces, owning_cards)),
        None => (owning_ifaces, owning_cards),
    };
    let port = find_interface(interface_id, ifaces, cards, manufacturer_cards)
        .map(|i| directional_port_name(i, side))
        .unwrap_or_else(|| sanitize_id(interface_id));
    PortRef {
        instance: instance_qualifier.map(|s| s.to_string()),
        port,
        index: Some(IndexSpec {
            elements: vec![IndexElement::Single { value: channel }],
        }),
    }
}

/// Returns true if the interface_id is an RF sentinel port (__rf_receive__, __rf_transmit__).
/// These are virtual ports emitted by older frontend versions to represent wireless paths.
/// They have no physical port in any template and must be dropped on emit.
pub(super) fn is_rf_sentinel(interface_id: &str) -> bool {
    interface_id.starts_with("__") && interface_id.ends_with("__")
}

pub(super) fn build_instance_buses(
    buses: &[BusEmitInput],
    ifaces: &[InterfaceEmitInput],
) -> Vec<BusEntry> {
    buses
        .iter()
        .map(|bus| {
            let bus_name = sanitize_id(&bus.label);

            let inputs: Vec<PortRef> = bus
                .input_channels
                .iter()
                .map(|ch| PortRef {
                    instance: None,
                    port: sanitize_id(&bus.input_interface),
                    index: Some(IndexSpec {
                        elements: vec![IndexElement::Single { value: *ch }],
                    }),
                })
                .collect();

            let outputs: Vec<BusOutput> = if bus.named_outputs.is_empty() {
                // Fallback: single unnamed output using flat output_interface + channels
                if bus.output_channels.is_empty() {
                    Vec::new()
                } else {
                    let dests = bus
                        .output_channels
                        .iter()
                        .map(|ch| PortRef {
                            instance: None,
                            port: sanitize_id(&bus.output_interface),
                            index: Some(IndexSpec {
                                elements: vec![IndexElement::Single { value: *ch }],
                            }),
                        })
                        .collect();
                    vec![BusOutput {
                        label: String::new(),
                        destinations: dests,
                        span: builder_span(),
                    }]
                }
            } else {
                bus.named_outputs
                    .iter()
                    .map(|out| {
                        let dests = out
                            .channels
                            .iter()
                            .map(|ch| PortRef {
                                instance: out.instance.clone(),
                                port: sanitize_id(&out.interface),
                                index: Some(IndexSpec {
                                    elements: vec![IndexElement::Single { value: *ch }],
                                }),
                            })
                            .collect();
                        BusOutput {
                            label: out.name.clone(),
                            destinations: dests,
                            span: builder_span(),
                        }
                    })
                    .collect()
            };

            let _ = ifaces; // used for future label resolution
            BusEntry {
                name: bus_name,
                label: bus
                    .display_name
                    .as_ref()
                    .filter(|n| !n.is_empty())
                    .cloned(),
                inputs,
                outputs,
                span: builder_span(),
            }
        })
        .collect()
}
