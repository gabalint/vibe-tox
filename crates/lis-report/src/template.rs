//! Versioned report templates embedded in the binary (REQ-RPT-007).

/// Identifies one immutable template version, for example `tox-report` v1.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemplateRef {
    /// Stable template family name.
    pub id: String,
    /// Version within the family; an issued version never changes.
    pub version: u32,
}

/// One embedded template version.
pub(crate) struct Template {
    pub(crate) id: &'static str,
    pub(crate) version: u32,
    pub(crate) source: &'static str,
}

/// Every template the renderer can use. Add a new version as a new file and a
/// new entry; never edit an existing file (its digest is pinned in the tests).
pub(crate) const TEMPLATES: &[Template] = &[
    Template {
        id: "tox-report",
        version: 1,
        source: include_str!("../templates/tox-report/v1.typ"),
    },
    Template {
        id: "tox-report",
        version: 2,
        source: include_str!("../templates/tox-report/v2.typ"),
    },
    Template {
        id: "tox-report",
        version: 3,
        source: include_str!("../templates/tox-report/v3.typ"),
    },
    Template {
        id: "tox-report-standard",
        version: 1,
        source: concat!(
            include_str!("../templates/tox-layouts/contract-v1.typ"),
            include_str!("../templates/tox-layouts/standard-v1.typ"),
        ),
    },
    Template {
        id: "tox-report-compact",
        version: 1,
        source: concat!(
            include_str!("../templates/tox-layouts/contract-v1.typ"),
            include_str!("../templates/tox-layouts/compact-v1.typ"),
        ),
    },
    Template {
        id: "tox-report-results-first",
        version: 1,
        source: concat!(
            include_str!("../templates/tox-layouts/contract-v1.typ"),
            include_str!("../templates/tox-layouts/results-first-v1.typ"),
        ),
    },
    Template {
        id: "report-notice",
        version: 1,
        source: include_str!("../templates/report-notice/v1.typ"),
    },
    Template {
        id: "catalog-preview",
        version: 1,
        source: include_str!("../templates/catalog-preview/v1.typ"),
    },
    Template {
        id: "catalog-preview",
        version: 2,
        source: include_str!("../templates/catalog-preview/v2.typ"),
    },
];

pub(crate) fn find(template: &TemplateRef) -> Option<&'static Template> {
    TEMPLATES
        .iter()
        .find(|candidate| candidate.id == template.id && candidate.version == template.version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::sha256_hex;

    /// An issued template must never change; a new layout is a new version.
    /// If this fails, revert the edit and add `vN+1.typ` instead.
    #[test]
    fn issued_template_digests_are_pinned() {
        let pinned = [
            (
                "tox-report",
                1,
                "84fba29cbbcf12065f9b01011b87ed3b91022fa555847c787e1bf52eccb971d2",
            ),
            (
                "tox-report",
                2,
                "87021e70f98d3d7b02f29856b2a1dc28ca62ace95a0cc4fab90fef6ce4f24a7d",
            ),
            (
                "tox-report",
                3,
                "8096b8f1de0b336cb91bba286b38164dcc781302c9f07663f76b19f27ef397d9",
            ),
            (
                "tox-report-standard",
                1,
                "f7bd2101200eb3c7d4fc0bf0e525722a20e43af9b25b798b8948d4e67e3a7406",
            ),
            (
                "tox-report-compact",
                1,
                "ec1885839b3657efd34d093e7810039433bdb1818453ac0ea41160af13a84861",
            ),
            (
                "tox-report-results-first",
                1,
                "ba4491dab1ff43f86b86366ab9d4585d529dcac023a110ec61d1a84c37171a24",
            ),
            (
                "report-notice",
                1,
                "192d85794158e6c59c8a3853afad2f45aefd4ce398d00478c78626ba202ab9aa",
            ),
            (
                "catalog-preview",
                1,
                "1e6fd90c96e346647294140e14191d07b4d12edc34793ba6fd42455d814ff670",
            ),
            (
                "catalog-preview",
                2,
                "e92ee55fa619a7ad44c9160f8c6cc47024dac90a81755a65a7863b19bc306c10",
            ),
        ];
        for (id, version, digest) in pinned {
            let template = find(&TemplateRef {
                id: id.to_owned(),
                version,
            })
            .expect("registered");
            assert_eq!(
                sha256_hex(template.source.as_bytes()),
                digest,
                "{id} v{version}"
            );
        }
    }

    #[test]
    fn template_versions_are_unique() {
        for (index, template) in TEMPLATES.iter().enumerate() {
            let duplicates = TEMPLATES[index + 1..]
                .iter()
                .filter(|other| other.id == template.id && other.version == template.version);
            assert_eq!(
                duplicates.count(),
                0,
                "{} v{}",
                template.id,
                template.version
            );
        }
    }
}
