use crate::ast::{
    BusEntry, BusOutput, IndexElement, IndexSpec, PortRef, RouteEntry,
};
use crate::builder::canvas_input::*;
use crate::builder::insert_endpoints::InsertEndpoint;
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

/// Convert insert endpoint DTOs to `PortRef`s — one per leg, in order (#31).
///
/// No grouping, no channel union, no dedup: `[L, R]` order is significant and a port
/// legitimately repeats across legs (`send: MADI[3], MADI[10]`).
///
/// Resolution goes through `resolve_route_endpoint`, the same path instance routes and
/// channel labels use, rather than a bare `sanitize_id`. That matters because
/// `should_split_io` (ports.rs) expands ONE frontend interface into TWO template ports
/// (`{base}_In` / `{base}_Out`) for any io/asymmetric interface that is not a ring/bus
/// protocol — MADI among them. The canonical insert case sends and returns on the SAME
/// interface, so the legs must emit as `MADI_Out[3]` and `MADI_In[4]`; only the side
/// parameter distinguishes them, and only Rust has it. Sanitizing alone would emit a
/// bare `MADI[3]` that the template never declares.
///
/// `side` is fixed by which list this is: a send leaves the device (Output), a return
/// comes back into it (Input).
///
/// The `unwrap_or_else(sanitize_id)` fallback inside `resolve_route_endpoint` is load-
/// bearing here: it lets already-resolved port names and slot-qualified `__` compounds
/// pass through untouched instead of being mangled.
#[allow(clippy::too_many_arguments)]
fn insert_legs_to_port_refs(
    legs: &[InsertEndpoint],
    side: PortSide,
    ifaces: &[InterfaceEmitInput],
    installed_cards: &[InstalledCardEmitInput],
    manufacturer_cards: &[CardEmitInput],
    all_instances: &[InstanceEmitInput],
) -> Vec<PortRef> {
    legs.iter()
        .map(|leg| {
            resolve_route_endpoint(
                &leg.port,
                leg.channel,
                leg.instance.as_deref(),
                ifaces,
                installed_cards,
                manufacturer_cards,
                all_instances,
                side,
            )
        })
        .collect()
}

/// Resolve insert legs for a channel label into the canonical string property form.
///
/// Labels carry their legs as a quoted string because a label property value holds at
/// most one port ref; buses get native grammar. Both go through the same resolution —
/// see [`insert_legs_to_port_refs`] for why sanitizing alone is not enough.
#[allow(clippy::too_many_arguments)]
pub(super) fn format_resolved_insert_legs(
    legs: &[InsertEndpoint],
    side: PortSide,
    ifaces: &[InterfaceEmitInput],
    installed_cards: &[InstalledCardEmitInput],
    manufacturer_cards: &[CardEmitInput],
    all_instances: &[InstanceEmitInput],
) -> String {
    let refs = insert_legs_to_port_refs(
        legs,
        side,
        ifaces,
        installed_cards,
        manufacturer_cards,
        all_instances,
    );
    crate::builder::insert_endpoints::format_port_ref_list(&refs)
}

pub(super) fn build_instance_buses(
    buses: &[BusEmitInput],
    ifaces: &[InterfaceEmitInput],
    installed_cards: &[InstalledCardEmitInput],
    manufacturer_cards: &[CardEmitInput],
    all_instances: &[InstanceEmitInput],
) -> Vec<BusEntry> {
    buses
        .iter()
        .map(|bus| {
            let bus_name = sanitize_id(&bus.label);

            let inputs: Vec<PortRef> = if bus.input_groups.is_empty() {
                // Legacy path: flat input_interface + input_channels
                bus.input_channels
                    .iter()
                    .map(|ch| PortRef {
                        instance: None,
                        port: sanitize_id(&bus.input_interface),
                        index: Some(IndexSpec {
                            elements: vec![IndexElement::Single { value: *ch }],
                        }),
                    })
                    .collect()
            } else {
                // New path: one PortRef per channel per group, with cross-device instance
                bus.input_groups
                    .iter()
                    .flat_map(|group| {
                        let port = sanitize_id(&group.interface);
                        let instance = group.instance.clone();
                        if group.channels.is_empty() {
                            vec![PortRef {
                                instance,
                                port,
                                index: Some(IndexSpec {
                                    elements: vec![IndexElement::Single { value: 1 }],
                                }),
                            }]
                        } else {
                            group.channels
                                .iter()
                                .map(|ch| PortRef {
                                    instance: instance.clone(),
                                    port: port.clone(),
                                    index: Some(IndexSpec {
                                        elements: vec![IndexElement::Single { value: *ch }],
                                    }),
                                })
                                .collect()
                        }
                    })
                    .collect()
            };

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

            BusEntry {
                name: bus_name,
                label: bus
                    .display_name
                    .as_ref()
                    .filter(|n| !n.is_empty())
                    .cloned(),
                inputs,
                outputs,
                // One PortRef per leg, in order (#31). No grouping, no channel union,
                // no dedup — unlike the inputs above, which are set-like.
                insert_send: insert_legs_to_port_refs(
                    &bus.insert_send,
                    PortSide::Output,
                    ifaces,
                    installed_cards,
                    manufacturer_cards,
                    all_instances,
                ),
                insert_return: insert_legs_to_port_refs(
                    &bus.insert_return,
                    PortSide::Input,
                    ifaces,
                    installed_cards,
                    manufacturer_cards,
                    all_instances,
                ),
                span: builder_span(),
            }
        })
        .collect()
}
