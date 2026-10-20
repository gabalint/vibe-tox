# ADR 0018: Configuration authority and continuity of accepted work

Status: **Accepted by the project owner, 2026-10-14.** Current implementation is summarized in the [implementation plan](../../planning/implementation-plan.md#status); unresolved capabilities remain in [scope decisions](../../quality/scope-decisions.md).

## Decision

Administrators approve and activate application configuration. The application records the administrator, exact configuration version, activation time and audit history; external laboratory approval references may be retained without a compulsory integrated QMS or duplicate clinical signoff. The existing versioned separation-of-duties policy ([ADR 0015](0015-separation-of-duties-policy.md)) still applies. Configuration approval grants no patient order, technical review or report release authority. Laboratory-approved content remains the laboratory's responsibility; the software never invents thresholds, conversions, diagnoses or interpretation.

The separation policy itself is application configuration: an administrator changes it using fresh authentication, a reason and the current version. Quality staff retain read access. Defaults, per-duty switches, confirmation requirements and immutable policy history remain enforced.

Routine method supersession changes the default for new orders. Accepted orders retain their frozen ordered scope and approved method versions, including creation of runs, preparation and repeat/re-extraction/dilution work after a newer version becomes current. Individual and coordinated activation must allow this continuity without draining outstanding work or silently repinning it. Explicit safety retirement and all identity, QC, validity, material and release gates remain effective. Original observations, approved versions and issued artifacts remain immutable.

Patient-specific standing authority is evaluated against collection evidence, never the acceptance command's UTC date. Date-only evidence uses the supplied civil date. An instant uses its retained source offset to obtain the collection civil date; the application does not reinterpret it in the current laboratory time zone. Valid-from and valid-until are inclusive. Unknown dates are retained but block testing and release until source-backed correction. Existing collection corrections and quality review remain attributable; quality review cannot extend an expired authorization.

Acceptance reserves one use under the existing visit-limit contract. Later cancellation does not recycle that reservation. Patient, practitioner and supplied scope checks remain mandatory; practice templates provide no independent authorization. Collection corrections re-evaluate effective authority. A valid earlier collection remains eligible after delayed intake or later testing, subject to other gates.

## Consequences

| Boundary | Current decision |
| --- | --- |
| Configuration authority | Administrator holds `approve_configuration`; quality management retains its clinical duties. |
| Method continuity | Still-approved superseded methods support frozen accepted work. Explicit safety retirement retains its separate controls. |
| Standing authority | Testing and release use supplied collection evidence; missing timing remains unresolved until source-backed correction. |
| Clinical content | Configurable implementation stays within US clinical scope. The laboratory supplies and approves actual content before use; state/accreditation applicability remains open. |

The capacity target is 100–250 patients per day; patient, accession, specimen, execution and result units are distinct. Measurement must include historical accumulation, peak imports, repeats and multi-analyte specimens before additional infrastructure is proposed.

DM-07 source custody, RPT-06 urgent/delay communication and RPT-07 patient/proxy requests remain separate product-scope decisions. Permission to maintain formal configuration approvals externally does not settle those retention and workflow questions.
