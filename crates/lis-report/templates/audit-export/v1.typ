// Audit trail export, template "audit-export" version 1 (REQ-OPS-001).
//
// Issued templates are immutable: change the layout or the data contract in
// a new version file, never here (REQ-RPT-007).
//
// The audit events that matched the filters of an export, for an auditor or
// an inspection: who did what, when, why, under which authority, and the old
// and new values the engine read from each record's history. The template is
// presentational: it prints what the engine supplies, in the order given.
//
// Inputs
// - sys.inputs.issued-at: supplied by the renderer to every template and not
//   used here. The printed export time is export.exportedAt, already
//   formatted by the engine in the laboratory's time zone.
// - /data.json: one JSON object; every scalar is a string.
//
//   laboratoryName
//   export:  { exportedAt, exportedBy, recordedBefore, eventCount,
//              patientData: "yes" | "no" }
//   filters: [{ name, value }]
//   events:  [{ stream, seq, occurred, recorded, by, action, outcome, record,
//              reason, authority, revision, changes, details, correlationId }]
//
// Rules
// - laboratoryName and every export value are non-empty; an event value may
//   be empty and prints as a dash.
// - With patientData "yes" every page says the document holds patient
//   information.

#let data = json("/data.json")

// A very long word cannot wrap, so it would run off the page: allow a line
// break (a zero-width space) between its characters.
#let breakable(value) = value.replace(
  regex("\S{24,}"),
  run => run.text.clusters().join("\u{200B}"),
)
// A string that must not be empty; anything else fails the render.
#let required(value) = {
  assert(type(value) == str, message: "contract: expected a string")
  assert(value != "", message: "contract: required value is empty")
  breakable(value)
}
// A string that may be empty, shown as a dash when it is.
#let or-dash(value) = {
  assert(type(value) == str, message: "contract: expected a string")
  if value == "" { "–" } else { breakable(value) }
}
#let list-of(value) = {
  assert(type(value) == array, message: "contract: expected a list")
  value
}

#assert(
  data.keys().sorted() == ("events", "export", "filters", "laboratoryName"),
  message: "contract: unexpected top-level keys",
)
#let export = data.export
#assert(
  export.keys().sorted()
    == ("eventCount", "exportedAt", "exportedBy", "patientData", "recordedBefore"),
  message: "contract: unexpected export keys",
)
#assert(
  export.patientData in ("yes", "no"),
  message: "contract: patientData is yes or no",
)
#let event-keys = (
  "action", "authority", "by", "changes", "correlationId", "details", "occurred",
  "outcome", "reason", "record", "recorded", "revision", "seq", "stream",
)
#let lab-name = required(data.laboratoryName)
#let patient-data = export.patientData == "yes"
#let confidential = "Contains patient information: confidential"

#set document(title: "Audit trail export")
#set text(lang: "en", region: "US", size: 7.5pt)
#set par(justify: false)
#set table(
  inset: 3pt,
  stroke: 0.5pt + luma(120),
  fill: (_, y) => if y == 0 { luma(235) },
)
#set page(
  paper: "us-letter",
  flipped: true,
  margin: (x: 12mm, top: 20mm, bottom: 16mm),
  header: [
    #set text(size: 8pt)
    #lab-name · Audit trail export #h(1fr)
    #if patient-data { strong(confidential) }
  ],
  footer: context [
    #set text(size: 8pt)
    Exported #required(export.exportedAt) by #required(export.exportedBy) #h(1fr)
    Page #counter(page).display("1 of 1", both: true)
  ],
)

= Audit trail export

#if patient-data [
  #block(stroke: 1.5pt, inset: 6pt, width: 100%)[
    #strong(confidential + ".") Keep and dispose of this document under the laboratory's
    procedure for patient records.
  ]
]

/ Laboratory: #lab-name
/ Exported: #required(export.exportedAt)
/ Exported by: #required(export.exportedBy)
/ Events: #required(export.eventCount), recorded up to #required(export.recordedBefore)

== Filters

#let filters = list-of(data.filters)
#if filters.len() == 0 [No filter: every event of every stream.] else [
  #for filter in filters [
    / #required(filter.name): #required(filter.value)
  ]
]

== Events

#table(
  columns: (5.5em, 6em, 7em, 8em, 4.5em, 9em, 1fr, 1.6fr),
  table.header(
    [*Stream · Seq.*], [*Recorded*], [*By*], [*Action*], [*Outcome*],
    [*Record*], [*Reason · Authority*], [*Changes*],
  ),
  ..list-of(data.events).map(event => {
    assert(event.keys().sorted() == event-keys, message: "contract: unexpected event keys")
    (
      [#required(event.stream) #required(event.seq)],
      [#required(event.recorded) \ #text(size: 6.5pt)[Occurred #required(event.occurred)]],
      [#or-dash(event.by)],
      [#required(event.action)],
      [#required(event.outcome)],
      [#or-dash(event.record)],
      [#or-dash(event.reason) \ #text(size: 6.5pt)[#or-dash(event.authority)]],
      [
        #if event.revision != "" [Revision #breakable(event.revision) \ ]
        #or-dash(event.changes)
        #if event.details != "" [\ #text(size: 6.5pt)[#breakable(event.details)]]
        \ #text(size: 6pt)[Correlation #or-dash(event.correlationId)]
      ],
    )
  }).flatten(),
)
