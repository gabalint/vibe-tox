// Report notice, template "report-notice" version 1.
//
// Issued templates are immutable: change the layout or the data contract in
// a new version file, never here (REQ-RPT-007).
//
// A withdrawal notice tells every earlier recipient that the laboratory has
// withdrawn a report. It carries a fixed statement and identifies the report
// only: no demographics, no results and no reason (REQ-RPT-002). Any key
// beyond the contract fails the render, so such data cannot be printed by
// mistake.
//
// Inputs
// - sys.inputs.issued-at: supplied by the renderer to every template and not
//   used here. The printed issued time is notice.issuedAt, already formatted
//   by the engine in the laboratory's time zone.
// - /data.json: one JSON object; every value is a non-empty string.
//
//   laboratory: { name, addressLines: [string], cliaId, director, phone }
//   notice:     { reportNumber, revision, accession, kind: "withdrawal",
//                 issuedAt, issuedBy }
//               `revision` is the report's latest revision when withdrawn.

#let data = json("/data.json")

#let kind-titles = (withdrawal: "Report withdrawal notice")

// A string that must not be empty; anything else fails the render.
#let required(value) = {
  assert(type(value) == str, message: "contract: expected a string")
  assert(value != "", message: "contract: required value is empty")
  value
}
#let list-of(value) = {
  assert(type(value) == array, message: "contract: expected a list")
  value
}

#assert(
  data.keys().sorted() == ("laboratory", "notice"),
  message: "contract: unexpected top-level keys",
)
#let notice = data.notice
#assert(
  notice.keys().sorted()
    == ("accession", "issuedAt", "issuedBy", "kind", "reportNumber", "revision"),
  message: "contract: unexpected notice keys",
)
#let title = kind-titles.at(notice.kind)
#let lab = data.laboratory

#set document(title: "Report withdrawal notice")
#set text(lang: "en", region: "US", size: 9.5pt)
#set par(justify: false)
#set page(
  paper: "us-letter",
  margin: (x: 16mm, top: 24mm, bottom: 20mm),
  header: [
    #set text(size: 8.5pt)
    #required(lab.name) #h(1fr)
    #title
  ],
  footer: context [
    #set text(size: 8.5pt)
    Report #required(notice.reportNumber) · Revision #required(notice.revision) ·
    #title #h(1fr)
    Page #counter(page).display("1 of 1", both: true)
  ],
)

= #title

#block(stroke: 1pt, inset: 6pt, width: 100%)[
  *The laboratory has withdrawn report #required(notice.reportNumber).*
  Revision #required(notice.revision) and every earlier revision of this
  report must not be used. Contact the laboratory with any questions.
]

/ Report: #required(notice.reportNumber)
/ Latest revision: #required(notice.revision)
/ Accession: #required(notice.accession)
/ Issued: #required(notice.issuedAt)
/ Issued by: #required(notice.issuedBy)

== Laboratory

/ Laboratory: #required(lab.name)
/ Address: #list-of(lab.addressLines).map(required).join(", ")
/ CLIA number: #required(lab.cliaId)
/ Director: #required(lab.director)
/ Telephone: #required(lab.phone)
