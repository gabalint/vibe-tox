// Clinical toxicology report, template "tox-report" version 3.
//
// Issued templates are immutable: change the layout or the data contract in
// a new version file, never here (REQ-RPT-007).
//
// The template is presentational. It performs no arithmetic, never compares a
// result with a cutoff and adds no interpretation: every clinical statement,
// including the comparison wording, arrives from the engine as text
// (REQ-RPT-008). A missing key, a value of the wrong type, an unknown code or
// a broken rule below fails the render.
//
// Inputs
// - sys.inputs.issued-at: supplied by the renderer to every template and not
//   used here. PDF metadata uses the renderer-supplied generation instant.
//   report.generatedAt is formatted in the laboratory time zone. It is the
//   preparation time, not the later release commit retained in LIS history.
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
//                    recordedAt (or null), firstReportedAt (or null),
//                    referenceInterval (or null), performingSites: [string] }] }]
//   otherTests:  [{ name, status: "pending" | "held" | "cancelled"
//                  | "not_activated" | "trigger_pending", note }]
//   validity:    { outcome, notes: [string] }
//   releasedBy:  { name } — retained wire name; printed "Prepared for release by".
//   Row recorded times describe retained result revisions, not inferred bench
//   completion times. First-report times identify earlier issued revisions;
//   null means this prepared revision, whose release time is kept in history.
//   Performing sites describe selected evidence, independently of issuer identity.
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

#let report = data.report
#let completeness-label = completeness-labels.at(report.completeness)
#let relationship-label = relationship-labels.at(report.relationship)
#let supersedes = report.supersedes
#let amendment-reason = report.amendmentReason
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

#set document(title: "Clinical toxicology report")
#set text(lang: "en", region: "US", size: 9.5pt)
#set par(justify: false)
#set table(inset: 4pt, stroke: 0.5pt + luma(150))
#let running-header = [
  #set text(size: 8.5pt)
  #grid(
    columns: (1fr, 2fr), gutter: 12pt,
    [#required(lab.name)],
    [#required(subject.name) #linebreak()
     Born #required(subject.birthDate) · Accession #required(specimen.accession)],
  )
]
// Header height follows the actual wrapped identity, so long supplied names
// never overlap the body. Letter width and side margins are fixed below.
#show: body => context {
  let header-height = measure(running-header, width: 8.5in - 32mm).height
  set page(
    paper: "us-letter",
    margin: (x: 16mm, top: calc.max(24mm, header-height + 18mm), bottom: 20mm),
    header-ascent: 6mm,
    footer-descent: 6mm,
    header: running-header,
    footer: context [
      #set text(size: 8.5pt)
      #grid(
        columns: (1fr, auto), gutter: 12pt,
        [Report #required(report.number) · Revision #required(report.revision) ·
         #completeness-label · #relationship-label],
        [Page #counter(page).display("1 of 1", both: true)],
      )
    ],
  )
  body
}

= Clinical toxicology report

#if report.relationship == "correction" [
  #block(stroke: 1pt, inset: 6pt, width: 100%)[
    *Correction.* This revision corrects and replaces revision
    #required(supersedes.revision), released #required(supersedes.releasedAt).
    Reason for correction: #required(amendment-reason)
  ]
] else if report.relationship == "supplement" [
  #block(stroke: 1pt, inset: 6pt, width: 100%)[
    *Supplement.* This revision adds results not reported in revision
    #required(supersedes.revision), released #required(supersedes.releasedAt).
  ]
]

/ Report: #required(report.number)
/ Revision: #required(report.revision)
/ Completeness: #completeness-label
/ Report type: #relationship-label
/ Generated: #required(report.at("generatedAt", default: none))

== Reporting laboratory contact

/ Laboratory: #required(lab.name)
/ Address: #list-of(lab.addressLines).map(required).join(", ")
/ CLIA number: #required(lab.cliaId)
/ Director: #required(lab.director)
/ Telephone: #required(lab.phone)

== Patient

/ Patient: #required(subject.name)
/ Birth date: #required(subject.birthDate)
/ Sex: #or-unknown(subject.sex)
#for identifier in list-of(subject.identifiers) [
  / #required(identifier.type): #required(identifier.value)
    (assigned by #required(identifier.authority))
]

== Order

/ Ordering practitioner: #required(order.practitioner.name)
/ NPI: #required(order.practitioner.npi)
/ Client: #required(order.client)
#if order.location != none [
  / Location: #required(order.location)
]
#if order.placerOrder != none [
  / Placer order: #required(order.placerOrder)
]

== Specimen

/ Accession: #required(specimen.accession)
/ Specimen type: #required(specimen.matrix)
/ Containers: #if list-of(specimen.containerLabels).len() == 0 [None] else {
  specimen.containerLabels.map(required).join(", ")
}
/ Collected: #or-unknown(specimen.collected)
/ Received: #or-unknown(specimen.received)
#for note in list-of(specimen.conditionNotes) [
  / Condition: #required(note)
]
#for observation in list-of(specimen.collectionObservations) [
  / #required(observation.code): #required(observation.value)
    (observed by #required(observation.observedBy))
]

== Medications as supplied with the order

#if list-of(data.medications).len() == 0 [
  None supplied.
] else [
  #for medication in data.medications [
    - #required(medication.name) (source: #required(medication.source),
      as of #or-unknown(medication.asOf))
  ]
]

#if report.relationship == "correction" [
  == Changes in this correction
  #for change in list-of(report.at("changeSummary", default: ())) [
    - #required(change)
  ]
]

== Results

#if list-of(data.resultGroups).len() == 0 [
  No results are reported in this revision.
]
#for group in data.resultGroups [
  === #required(group.name)
  / Method: #required(group.method)
  / Method certainty: #required(group.certainty)
  / Role: #required(group.role)
  #if group.at("reportNotes", default: none) != none [
    / Method information: #required(group.at("reportNotes", default: none))
  ]
  #table(
    columns: (1.6fr, 1fr, 0.8fr, 1.4fr, 1.2fr),
    table.header(
      [*Analyte*], [*Result*], [*Units*], [*Limits*], [*Comparison*],
    ),
    ..list-of(group.rows)
      .map(row => (
        [#required(row.analyte) #linebreak() #text(size: 7.5pt)[Result recorded: #or-unknown(row.at("recordedAt", default: none))]],
        [#required(row.result)],
        [#s(row.units)],
        [#required(row.limits)],
        [#required(row.comparison)],
      ))
      .flatten(),
  )
  #for row in group.rows [
    #if row.at("referenceInterval", default: none) != none [
      / Reference information — #required(row.analyte): #required(row.at("referenceInterval", default: none))
    ]
    / First reported — #required(row.analyte): #if row.at("firstReportedAt", default: none) == none { "This revision; actual release time in LIS history" } else { required(row.at("firstReportedAt", default: none)) }
    / Performing location — #required(row.analyte): #list-of(row.at("performingSites", default: ())).map(required).join("; ")
  ]
]

== Tests not reported

#if other-tests.len() == 0 [
  None.
] else [
  #table(
    columns: (2fr, 1.2fr, 2fr),
    table.header([*Test*], [*Status*], [*Note*]),
    ..other-tests
      .zip(other-test-statuses)
      .map(((test, status)) => ([#required(test.name)], [#status], [#s(test.note)]))
      .flatten(),
  )
]

== Specimen validity

/ Outcome: #required(data.validity.outcome)
#for note in list-of(data.validity.notes) [
  - #required(note)
]

== Preparation

/ Prepared for release by: #required(data.releasedBy.name)
/ Generated: #required(report.at("generatedAt", default: none))

The generation time describes this immutable prepared copy. Actual release time is retained in the LIS report history.
