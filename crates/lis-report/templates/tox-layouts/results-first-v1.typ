
// Layout "Results first", version 1 (follows the shared contract above): the
// patient strip, then every result as a large card per ordered component, then
// the order, specimen and other details, and the result notes.

#let accent = rgb("#5b2a86")
#let muted = luma(95)
#let rule = luma(210)

#show: layout-page.with(accent)
#show heading.where(level: 1): set text(size: 15pt)
#show heading.where(level: 2): set text(size: 11pt, fill: accent)
#show heading.where(level: 3): it => block(above: 8pt, below: 5pt)[
  #set text(size: 7.5pt, weight: "bold", fill: accent, tracking: 0.6pt)
  #upper(it.body)
]

#let strip-cell(label, value) = [
  #text(size: 6.5pt, fill: muted, tracking: 0.4pt, upper(label)) \
  #text(size: 9.5pt, weight: "bold", value)
]
#let result-card(row) = block(
  stroke: 0.6pt + rule, radius: 4pt, inset: 8pt, width: 100%, breakable: false,
)[
  #text(size: 9pt, weight: "bold", required(row.analyte)) \
  #text(size: 17pt, weight: "bold", required(row.result))
  #text(size: 7.5pt, fill: accent, weight: "bold", s(row.units)) \
  #text(size: 7pt, fill: muted)[#required(row.limits) · recorded
    #or-unknown(optional-key(row, "recordedAt"))]
  #v(1pt)
  #comparison-label(row, size: 7pt)
]

#grid(
  columns: (1fr, auto), gutter: 12pt,
  [
    #heading(level: 1)[Clinical Toxicology Report]
    #text(size: 7.5pt, fill: muted)[#required(lab.name) · #lab-line]
  ],
  align(right)[
    #text(size: 8pt)[Report] \
    #text(size: 13pt, weight: "bold", required(report.number)) \
    #text(size: 7.5pt)[Revision #required(report.revision) · #completeness-label ·
      #relationship-label]
  ],
)

#relationship-notice(accent)

#block(
  fill: rgb("#f3eff8"), stroke: (left: 3pt + accent), inset: (x: 10pt, y: 7pt),
  width: 100%, above: 10pt,
)[
  #grid(
    columns: (1.5fr, 1fr, 1fr, 1fr), gutter: 8pt,
    strip-cell("Patient", required(subject.name)),
    strip-cell("Born", required(subject.birthDate)),
    strip-cell("Sex", or-unknown(subject.sex)),
    strip-cell("Accession", required(specimen.accession)),
  )
  #for identifier in list-of(subject.identifiers) [
    #text(size: 7pt)[#required(identifier.type) #required(identifier.value)
      (assigned by #required(identifier.authority))]
  ]
]

#if result-groups.len() == 0 [
  == Results
  No results are reported in this revision.
]
#for group in result-groups [
  == Results — #required(group.name)
  #text(size: 7.5pt, fill: muted)[Method #required(group.method) · Certainty
    #required(group.certainty) · Role #required(group.role)]
  #grid(
    columns: (1fr, 1fr, 1fr), gutter: 8pt,
    ..group.rows.map(result-card),
  )
]

== Details
#line(length: 100%, stroke: 0.5pt + rule)
#grid(
  columns: (1fr, 1fr, 1fr), gutter: 12pt,
  [
    === Order
    #facts(order-facts, label-fill: muted, gutter: 3pt)
  ],
  [
    === Specimen
    #facts(specimen-facts.slice(1), label-fill: muted, gutter: 3pt)
  ],
  [
    === Other
    #facts(
      (
        ("Medications", medications-text),
        ("Tests not reported", other-tests-text),
        ("Specimen validity", validity-text),
        ("Prepared by", required(data.releasedBy.name)),
        ("Generated", generated-at),
      ),
      label-fill: muted,
      gutter: 3pt,
    )
  ],
)

#if result-notes.len() > 0 [
  === Result notes
  #set text(size: 7.5pt)
  #for note in result-notes [
    - #note
  ]
]

#text(size: 7pt, fill: muted, preparation-statement)

#align(center, text(size: 7pt, fill: muted)[— End of report —])
