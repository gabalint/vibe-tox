// Clinical toxicology report, template "tox-report" version 1.
//
// Issued templates are immutable: change the layout or the data contract in
// a new version file, never here (REQ-RPT-007).
//
// The template is presentational. It performs no arithmetic, never compares a
// result with a cutoff and adds no interpretation: every clinical statement
// arrives from the engine as text (REQ-RPT-008). A missing key, a value of the
// wrong type or an unknown code fails the render.
//
// Inputs
// - sys.inputs.issued-at: the issued time in UTC, formatted by the renderer
//   as "YYYY-MM-DD hh:mm:ss UTC".
// - /data.json: one JSON object. Every scalar is a string, shown exactly as
//   given; JSON numbers and booleans are refused before rendering. `null` is
//   allowed only where marked "or null" and prints as "Unknown".
//
//   report:      { id, status: "preliminary" | "final" | "corrected", revision,
//                  amendment: null, or for "corrected" only
//                  { supersedes_revision, reason } }
//   laboratory:  { name, address_lines: [string], clia_id, director, phone }
//   subject:     { name, birth_date,
//                  identifiers: [{ type, value, authority }] }
//   specimen:    { accession, specimen_id, matrix,
//                  collected_at (or null), received_at (or null) }
//   ordering_practitioner: { name, npi, organization }
//   medications: [{ name, source, as_of (or null) }]   as supplied by the order
//   result_groups: [{ name, rows: [{ analyte, result, units, cutoff,
//                  method_certainty: "presumptive" | "definitive",
//                  flags: [string] }] }]
//                  `result` is the exact lexical result; `cutoff` must not be
//                  empty; `units` may be empty for qualitative results.
//   validity:    { outcome, notes: [string] }
//   comments:    [{ text, source }]   source names the rule version or author
//   signatory:   { name, title }

#let data = json("/data.json")
#let issued-at = sys.inputs.at("issued-at")

#let status-labels = (
  preliminary: "Preliminary report",
  final: "Final report",
  corrected: "Corrected report",
)
#let certainty-labels = (presumptive: "Presumptive", definitive: "Definitive")

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
#let or-unknown(value) = if value == none { "Unknown" } else { s(value) }
#let list-of(value) = {
  assert(type(value) == array, message: "contract: expected a list")
  value
}
#let joined(values) = if list-of(values).len() == 0 { "None" } else {
  values.map(s).join(", ")
}

#let report = data.report
#let status-label = status-labels.at(report.status)
#let amendment = report.amendment
#assert(
  (report.status == "corrected") == (amendment != none),
  message: "contract: an amendment is required exactly for corrected reports",
)
#let lab = data.laboratory
#let subject = data.subject
#let specimen = data.specimen
#let practitioner = data.ordering_practitioner

#set document(title: "Clinical toxicology report")
#set text(lang: "en", region: "US", size: 9.5pt)
#set par(justify: false)
#set table(inset: 4pt, stroke: 0.5pt + luma(150))
#set page(
  paper: "us-letter",
  margin: (x: 16mm, top: 24mm, bottom: 20mm),
  header: [
    #set text(size: 8.5pt)
    #required(lab.name) #h(1fr)
    #required(subject.name) · Born #required(subject.birth_date) ·
    Accession #required(specimen.accession)
  ],
  footer: context [
    #set text(size: 8.5pt)
    Report #required(report.id) · Revision #required(report.revision) ·
    #status-label #h(1fr)
    Page #counter(page).display("1 of 1", both: true)
  ],
)

= Clinical toxicology report: #status-label

#if amendment != none [
  #block(stroke: 1pt, inset: 6pt, width: 100%)[
    *Corrected report.* Replaces revision
    #required(amendment.supersedes_revision).
    Reason for correction: #required(amendment.reason)
  ]
]

/ Report: #required(report.id), revision #required(report.revision)
/ Status: #status-label
/ Issued: #issued-at

== Performing laboratory

/ Laboratory: #required(lab.name)
/ Address: #list-of(lab.address_lines).map(required).join(", ")
/ CLIA number: #required(lab.clia_id)
/ Director: #required(lab.director)
/ Telephone: #required(lab.phone)

== Patient and specimen

/ Patient: #required(subject.name)
/ Birth date: #required(subject.birth_date)
#for identifier in list-of(subject.identifiers) [
  / #required(identifier.type): #required(identifier.value)
    (assigned by #required(identifier.authority))
]
/ Accession: #required(specimen.accession)
/ Specimen: #required(specimen.specimen_id)
/ Specimen type: #required(specimen.matrix)
/ Collected: #or-unknown(specimen.collected_at)
/ Received: #or-unknown(specimen.received_at)

== Ordering practitioner

/ Practitioner: #required(practitioner.name)
/ NPI: #required(practitioner.npi)
/ Organization: #required(practitioner.organization)

== Medications as supplied with the order

#if list-of(data.medications).len() == 0 [
  None supplied.
] else [
  #for medication in data.medications [
    - #required(medication.name) (source: #required(medication.source),
      as of #or-unknown(medication.as_of))
  ]
]

== Results

#for group in list-of(data.result_groups) [
  === #required(group.name)
  #table(
    columns: (1.6fr, 1.2fr, 0.8fr, 0.8fr, 1fr, 1.2fr),
    table.header(
      [*Analyte*], [*Result*], [*Units*], [*Cutoff*],
      [*Method certainty*], [*Flags*],
    ),
    ..list-of(group.rows)
      .map(row => (
        [#required(row.analyte)],
        [#required(row.result)],
        [#s(row.units)],
        [#required(row.cutoff)],
        [#certainty-labels.at(row.method_certainty)],
        [#joined(row.flags)],
      ))
      .flatten(),
  )
]

== Specimen validity

/ Outcome: #required(data.validity.outcome)
#for note in list-of(data.validity.notes) [
  - #required(note)
]

== Comments

#if list-of(data.comments).len() == 0 [
  None.
] else [
  #for comment in data.comments [
    - #required(comment.text) (source: #required(comment.source))
  ]
]

== Signature

/ Signatory: #required(data.signatory.name), #required(data.signatory.title)
/ Issued: #issued-at
