#![allow(dead_code, unused_imports)]

use std::collections::HashMap;
use super::easyschematic::{EsDeviceData, EsPort, RawNode};
use super::mapping::sanitize_identifier;

/// Fingerprint of a port set for deduplication within a templateId group.
///
/// Order-sensitive: port lists with the same ports in different order are
/// treated as distinct variants, keeping positional zip correct in Task 5.
fn port_set_fingerprint(ports: &[EsPort]) -> String {
    ports
        .iter()
        .map(|p| format!("{}:{}:{}", p.label, p.direction, p.signal_type))
        .collect::<Vec<_>>()
        .join("|")
}

pub(super) struct TemplateSpec {
    pub(super) name: String,
    pub(super) ports: Vec<EsPort>,
}

pub(super) struct TemplateAssignments {
    pub(super) specs: Vec<TemplateSpec>,
    /// Maps EasySchematic node ID → PatchLang template name.
    pub(super) node_to_template: HashMap<String, String>,
}

/// Determine which template name each device node maps to.
///
/// Groups by `templateId` (no-templateId nodes each form their own group).
/// Within each group, sub-groups by port set fingerprint. Variants get
/// `_v2`, `_v3` … suffixes. BTreeMap gives deterministic iteration order
/// so which variant gets the base name is reproducible across runs.
pub(super) fn build_template_assignments(
    device_nodes: &[(RawNode, EsDeviceData)],
) -> TemplateAssignments {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, (node, dev)) in device_nodes.iter().enumerate() {
        let key = dev
            .template_id
            .clone()
            .unwrap_or_else(|| format!("__no_tmpl_{}", node.id));
        groups.entry(key).or_default().push(i);
    }

    let mut specs: Vec<TemplateSpec> = Vec::new();
    let mut used_base_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut node_to_template: HashMap<String, String> = HashMap::new();

    for indices in groups.values() {
        let (_, first_dev) = &device_nodes[indices[0]];
        let raw_name = first_dev.model.as_deref().unwrap_or(&first_dev.label);
        let base = sanitize_identifier(raw_name);

        let base = if used_base_names.contains(&base) {
            let mut n = 2u32;
            loop {
                let candidate = format!("{}_{}", base, n);
                if !used_base_names.contains(&candidate) {
                    break candidate;
                }
                n += 1;
            }
        } else {
            base
        };
        used_base_names.insert(base.clone());

        let mut fingerprint_to_variant: HashMap<String, (String, Vec<EsPort>)> = HashMap::new();
        let mut variant_counter = 1u32;

        for &idx in indices {
            let (node, dev) = &device_nodes[idx];
            let fp = port_set_fingerprint(&dev.ports);

            let (variant_name, _) = fingerprint_to_variant
                .entry(fp)
                .or_insert_with(|| {
                    let name = if variant_counter == 1 {
                        base.clone()
                    } else {
                        format!("{}_v{}", base, variant_counter)
                    };
                    variant_counter += 1;
                    (name, dev.ports.clone())
                });

            node_to_template.insert(node.id.clone(), variant_name.clone());
        }

        for (name, ports) in fingerprint_to_variant.into_values() {
            specs.push(TemplateSpec { name, ports });
        }
    }

    TemplateAssignments { specs, node_to_template }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::easyschematic::{EsDeviceData, RawNode, RawPosition};

    fn make_device_node_with_ports(
        id: &str,
        template_id: Option<&str>,
        model: Option<&str>,
        ports: &[(&str, &str)],
    ) -> (RawNode, EsDeviceData) {
        let port_values: Vec<serde_json::Value> = ports
            .iter()
            .map(|(pid, plabel)| serde_json::json!({
                "id": pid, "label": plabel,
                "signalType": "analog-audio", "direction": "output"
            }))
            .collect();
        let data_val = serde_json::json!({
            "label": id,
            "model": model,
            "templateId": template_id,
            "ports": port_values
        });
        let node = RawNode {
            id: id.to_string(),
            node_type: "device".to_string(),
            position: RawPosition { x: 0.0, y: 0.0 },
            data: data_val.clone(),
            parent_id: None,
        };
        let data = EsDeviceData::from_value(&data_val).unwrap();
        (node, data)
    }

    #[test]
    fn two_identical_templates_share_one_spec() {
        let (n1, d1) = make_device_node_with_ports("tv-1", Some("tmpl-tv"), Some("TV"), &[("p1", "HDMI In")]);
        let (n2, d2) = make_device_node_with_ports("tv-2", Some("tmpl-tv"), Some("TV"), &[("p2", "HDMI In")]);
        let a = build_template_assignments(&[(n1, d1), (n2, d2)]);
        assert_eq!(a.specs.len(), 1, "identical port sets → one template spec");
        assert_eq!(a.node_to_template["tv-1"], a.node_to_template["tv-2"]);
    }

    #[test]
    fn different_port_counts_emit_variant_templates() {
        let (n1, d1) = make_device_node_with_ports(
            "mixer-a", Some("tmpl-mixer"), Some("Mixer"),
            &[("p1", "Out 1"), ("p2", "Out 2")],
        );
        let (n2, d2) = make_device_node_with_ports(
            "mixer-b", Some("tmpl-mixer"), Some("Mixer"),
            &[("p3", "Out 1"), ("p4", "Out 2"), ("p5", "Out 3")],
        );
        let a = build_template_assignments(&[(n1, d1), (n2, d2)]);
        assert_eq!(a.specs.len(), 2, "different port counts → two variants");
        let t1 = &a.node_to_template["mixer-a"];
        let t2 = &a.node_to_template["mixer-b"];
        assert_ne!(t1, t2);
        assert!(t1.ends_with("_v2") || t2.ends_with("_v2"));
    }

    #[test]
    fn no_template_id_gets_own_spec() {
        let (n1, d1) = make_device_node_with_ports(
            "custom-dev", None, Some("My Custom Device"), &[("p1", "Out 1")],
        );
        let a = build_template_assignments(&[(n1, d1)]);
        assert_eq!(a.specs.len(), 1);
        assert!(!a.node_to_template["custom-dev"].is_empty());
    }

    #[test]
    fn template_names_are_valid_identifiers() {
        let (n1, d1) = make_device_node_with_ports(
            "d1", Some("tmpl-1"), Some("Mac Studio (M4)"), &[],
        );
        let a = build_template_assignments(&[(n1, d1)]);
        let name = &a.node_to_template["d1"];
        assert!(name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_'));
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }
}
