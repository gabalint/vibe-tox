// Clinical toxicology report layouts, shared contract version 1.
//
// Each layout template ("tox-report-standard", "tox-report-compact",
// "tox-report-results-first", version 1) is this file followed by its layout
// file, joined when the binary is built. The joined source is the issued
// template and its digest is pinned: never edit this file once issued; a
// change is a new contract version and new layout versions (REQ-RPT-007).
//
// The layouts differ only in arrangement and styling. They print the same
// content as "tox-report" v3: every key below, with the same rules.
//
// The template is presentational. It performs no arithmetic, never compares a
// result with a cutoff and adds no interpretation: every clinical statement,
// including the comparison wording, arrives from the engine as text
// (REQ-RPT-008). The comparison code only chooses the label's colour; the
// printed words are always the engine's. A missing key, a value of the wrong
// type, an unknown code or a broken rule below fails the render.
//
// Inputs
// - sys.inputs.issued-at: supplied by the renderer and not used here. The
//   printed generation time is report.generatedAt, formatted by the engine in
//   the laboratory time zone; it is the preparation time, not the release.
// - /data.json: one JSON object. Every scalar is a string, shown exactly as
//   given; JSON numbers and booleans are refused before rendering. `null` is
//   allowed only where marked "or null".
//
//   report:      { number, revision,
//                  completeness: "partial" | "final",
//                  relationship: "initial" | "supplement" | "correction",
//                  supersedes: { revision, releasedAt } or null,
//                  amendmentReason (or null), generatedAt, changeSummary: [string],
//                  issuedAt (legacy compatibility field, not printed) }
//   laboratory:  { name, addressLines: [string], cliaId, director, phone }
//   subject:     { name, birthDate, sex (or null),
//                  identifiers: [{ type, value, authority }] }
//   order:       { practitioner: { name, npi }, client,
//                  location (or null), placerOrder (or null) }
//   specimen:    { accession, matrix, containerLabels: [string],
//                  collected (or null), received (or null),
//                  conditionNotes: [string],
//                  collectionObservations: [{ code, value, observedBy }] }
//   medications: [{ name, source, asOf (or null) }]   as supplied by the order
//   resultGroups: [{ name, method, certainty, role, reportNotes (or null),
//                  rows: [{ analyte, result, units, limits, comparison,
//                    comparisonCode: "meets_cutoff" | "does_not_meet_cutoff"
//                      | "not_classifiable" (or null),
//                    recordedAt (or null), firstReportedAt (or null),
//                    referenceInterval (or null), performingSites: [string] }] }]
//   otherTests:  [{ name, status: "pending" | "held" | "cancelled"
//                  | "not_activated" | "trigger_pending", note }]
//   validity:    { outcome, notes: [string] }
//   releasedBy:  { name } — printed "Prepared for release by".
//
// Rules
// - "initial" has no `supersedes`; "supplement" and "correction" require it.
// - `amendmentReason` is given exactly for "correction".
// - "partial" exactly when some other test is "pending", "held" or
//   "trigger_pending"; so "partial" requires a non-empty `otherTests`.
// - `limits` is never empty. `units` and `note` may be empty.

#let data = json("/data.json")

#let completeness-labels = (
  partial: "Partial report",
  final: "Final report",
)
#let relationship-labels = (
  initial: "Initial report",
  supplement: "Supplement",
  correction: "Correction",
)
#let other-test-labels = (
  pending: "Pending",
  held: "On hold",
  cancelled: "Cancelled",
  not_activated: "Not activated",
  trigger_pending: "Reflex decision pending",
)
#let open-statuses = ("pending", "held", "trigger_pending")
// Label colours by comparison code: (fill, border, text).
#let comparison-tones = (
  meets_cutoff: (rgb("#fdecec"), rgb("#f3b4b4"), rgb("#a3221b")),
  does_not_meet_cutoff: (rgb("#e8f5ec"), rgb("#a9d8b6"), rgb("#1d6b35")),
  not_classifiable: (luma(240), luma(200), luma(60)),
)

// A displayed string value; anything else fails the render.
#let s(value) = {
  assert(type(value) == str, message: "contract: expected a string")
  value
}
// A string that must not be empty.
#let required(value) = {
  assert(s(value) != "", message: "contract: required value is empty")
  value
}
// A string, or null shown explicitly as unknown.
#let or-unknown(value) = if value == none { "Unknown" } else { required(value) }
#let list-of(value) = {
  assert(type(value) == array, message: "contract: expected a list")
  value
}
#let optional-key(record, key) = record.at(key, default: none)

#let report = data.report
#let completeness-label = completeness-labels.at(report.completeness)
#let relationship-label = relationship-labels.at(report.relationship)
#let supersedes = report.supersedes
#let amendment-reason = report.amendmentReason
#let generated-at = required(optional-key(report, "generatedAt"))
#let change-summary = list-of(report.at("changeSummary", default: ()))
#let other-tests = list-of(data.otherTests)
#let other-test-statuses = other-tests.map(test => other-test-labels.at(test.status))

#assert(
  (report.relationship == "initial") == (supersedes == none),
  message: "contract: supersedes is required exactly for supplements and corrections",
)
#assert(
  (report.relationship == "correction") == (amendment-reason != none),
  message: "contract: an amendment reason is required exactly for corrections",
)
#assert(
  (report.completeness == "partial")
    == other-tests.any(test => test.status in open-statuses),
  message: "contract: partial exactly when a test is pending, held or awaiting a reflex decision",
)

#let lab = data.laboratory
#let subject = data.subject
#let order = data.order
#let specimen = data.specimen
#let medications = list-of(data.medications)
#let result-groups = list-of(data.resultGroups)
#let validity = data.validity

#for group in result-groups {
  for row in list-of(group.rows) {
    let _ = (required(row.analyte), required(row.result), s(row.units))
    let _ = (required(row.limits), required(row.comparison))
    let code = optional-key(row, "comparisonCode")
    assert(
      code == none or code in comparison-tones,
      message: "contract: unknown comparison code",
    )
  }
}

#let lab-address = list-of(lab.addressLines).map(required).join(", ")
#let lab-line = (
  lab-address,
  "Tel " + required(lab.phone),
  "CLIA " + required(lab.cliaId),
  "Director " + required(lab.director),
).join(" · ")
#let containers = if list-of(specimen.containerLabels).len() == 0 { "None" } else {
  specimen.containerLabels.map(required).join(", ")
}
#let footer-line = (
  required(lab.name),
  "Report " + required(report.number),
  "Revision " + required(report.revision),
  completeness-label,
  relationship-label,
).join(" · ")

// The engine's comparison wording as a coloured label; uncoloured without a code.
#let comparison-label(row, size: 7.5pt) = {
  let code = optional-key(row, "comparisonCode")
  let wording = required(row.comparison)
  if code == none {
    text(size: size, wording)
  } else {
    let (fill, border, ink) = comparison-tones.at(code)
    box(
      fill: fill,
      stroke: 0.6pt + border,
      radius: 6pt,
      inset: (x: 5pt, y: 2.5pt),
      text(size: size, weight: "bold", fill: ink, wording),
    )
  }
}

// Label/value rows for a small facts grid.
#let facts(pairs, label-fill: luma(90), gutter: 4pt) = grid(
  columns: (1fr, 2fr),
  column-gutter: 8pt,
  row-gutter: gutter,
  ..pairs
    .map(((label, value)) => (
      text(fill: label-fill, label),
      text(weight: "bold", value),
    ))
    .flatten(),
)

#let subject-facts = {
  let pairs = (
    ("Name", required(subject.name)),
    ("Birth date", required(subject.birthDate)),
    ("Sex", or-unknown(subject.sex)),
  )
  for identifier in list-of(subject.identifiers) {
    pairs.push((
      required(identifier.type),
      required(identifier.value) + " (assigned by " + required(identifier.authority) + ")",
    ))
  }
  pairs
}
#let order-facts = {
  let pairs = (
    ("Practitioner", required(order.practitioner.name)),
    ("NPI", required(order.practitioner.npi)),
    ("Client", required(order.client)),
  )
  if order.location != none { pairs.push(("Location", required(order.location))) }
  if order.placerOrder != none { pairs.push(("Placer order", required(order.placerOrder))) }
  pairs
}
#let specimen-facts = {
  let pairs = (
    ("Accession", required(specimen.accession)),
    ("Type", required(specimen.matrix)),
    ("Container", containers),
    ("Collected", or-unknown(specimen.collected)),
    ("Received", or-unknown(specimen.received)),
  )
  for note in list-of(specimen.conditionNotes) { pairs.push(("Condition", required(note))) }
  for observation in list-of(specimen.collectionObservations) {
    pairs.push((
      required(observation.code),
      required(observation.value) + " (observed by " + required(observation.observedBy) + ")",
    ))
  }
  pairs
}

// The boxed notice of a supplement or correction, or nothing.
#let relationship-notice(accent) = {
  if report.relationship == "correction" {
    block(stroke: 1pt + accent, inset: 7pt, radius: 3pt, width: 100%)[
      *Correction.* This revision corrects and replaces revision
      #required(supersedes.revision), released #required(supersedes.releasedAt).
      Reason for correction: #required(amendment-reason)
      #if change-summary.len() > 0 [
        #linebreak() *Changes in this correction:*
        #for change in change-summary [
          - #required(change)
        ]
      ]
    ]
  } else if report.relationship == "supplement" {
    block(stroke: 1pt + accent, inset: 7pt, radius: 3pt, width: 100%)[
      *Supplement.* This revision adds results not reported in revision
      #required(supersedes.revision), released #required(supersedes.releasedAt).
    ]
  }
}

// One line per result: first report, reference information and performing site.
#let result-notes = {
  let notes = ()
  for group in result-groups {
    if optional-key(group, "reportNotes") != none {
      notes.push([*#required(group.name)* method information:
        #required(optional-key(group, "reportNotes"))])
    }
    for row in group.rows {
      let first = optional-key(row, "firstReportedAt")
      let reported = if first == none {
        "first reported in this revision (actual release time in LIS history)"
      } else { "first reported " + required(first) }
      let sites = list-of(row.at("performingSites", default: ())).map(required)
      let performed = if sites.len() == 0 { "" } else { ". Performed at " + sites.join("; ") }
      let reference = optional-key(row, "referenceInterval")
      let reference-text = if reference == none { "" } else {
        ". Reference information: " + required(reference)
      }
      notes.push([*#required(row.analyte)*: #reported#performed#reference-text.])
    }
  }
  notes
}

#let medication-lines = medications.map(medication => {
  let source = required(medication.source)
  let as-of = or-unknown(medication.asOf)
  required(medication.name) + " (source: " + source + ", as of " + as-of + ")"
})

#let other-test-lines = other-tests.zip(other-test-statuses).map(((test, status)) => {
  let note = s(test.note)
  required(test.name) + ": " + status + (if note == "" { "" } else { " — " + note })
})

#let validity-notes = list-of(validity.notes).map(required)

// One-line forms for the dense layouts.
#let medications-text = if medication-lines.len() == 0 { "None supplied" } else {
  medication-lines.join("; ")
}
#let other-tests-text = if other-test-lines.len() == 0 { "None" } else {
  other-test-lines.join("; ")
}
#let validity-text = ((required(validity.outcome),) + validity-notes).join("; ")

#let preparation-statement = [The generation time describes this immutable
  prepared copy. Actual release time is retained in the LIS report history.]

// The running footer of every layout.
#let page-footer = context [
  #set text(size: 7.5pt, fill: luma(90))
  #line(length: 100%, stroke: 0.4pt + luma(200))
  #grid(
    columns: (1fr, auto), gutter: 12pt,
    footer-line,
    [Page #counter(page).display("1 of 1", both: true)],
  )
]

// The running identity: laboratory, patient, birth date and accession.
#let identity-line(accent) = grid(
  columns: (1fr, 2fr), gutter: 12pt,
  text(size: 7.5pt, weight: "bold", fill: accent, required(lab.name)),
  align(right, text(size: 7.5pt)[#required(subject.name) · Born #required(subject.birthDate) ·
    Accession #required(specimen.accession)]),
)

// US Letter pages with the running footer, and the running identity from page 2 on. The top
// margin follows the identity's wrapped height, so long supplied names never overlap the body.
#let layout-page(accent, x: 16mm, top: 18mm, body) = context {
  let height = measure(identity-line(accent), width: 8.5in - 2 * x).height
  set page(
    paper: "us-letter",
    margin: (x: x, top: calc.max(top, height + 11mm), bottom: 20mm),
    header-ascent: 5mm,
    footer-descent: 5mm,
    header: context if counter(page).get().first() > 1 { identity-line(accent) },
    footer: page-footer,
  )
  body
}

#set document(title: "Clinical toxicology report")
#set text(lang: "en", region: "US", size: 8.5pt)
#set par(justify: false)
