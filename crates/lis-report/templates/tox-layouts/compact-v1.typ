
// Layout "Compact", version 1 (follows the shared contract above): a dense
// report meant to fit one page — a patient strip at the top, order, collection
// and other details in three columns, one results table per ordered component
// and the notes in one box.

#let accent = rgb("#1f5c47")
#let muted = luma(95)
#let rule = luma(205)

#show: layout-page.with(accent, x: 14mm, top: 16mm)
#set text(size: 8pt)
#show heading.where(level: 1): set text(size: 9pt, weight: "regular")
#show heading.where(level: 2): it => block(above: 9pt, below: 5pt)[
  #set text(size: 7.5pt, weight: "bold", fill: accent, tracking: 0.6pt)
  #upper(it.body)
]

#let strip-cell(label, value) = [
  #text(size: 6.5pt, fill: muted, tracking: 0.4pt, upper(label)) \
  #text(size: 9pt, weight: "bold", value)
]

#grid(
  columns: (1fr, auto), gutter: 12pt,
  [
    #text(size: 15pt, weight: "bold", fill: accent, required(lab.name)) \
    #text(size: 7pt, fill: muted, lab-line)
  ],
  align(right)[
    #heading(level: 1)[Clinical toxicology report]
    #text(size: 11pt, weight: "bold", required(report.number)) · Rev #required(report.revision) \
    #text(size: 7pt, fill: muted)[#completeness-label · #relationship-label · Generated #generated-at]
  ],
)
#v(-2pt)
#line(length: 100%, stroke: 1.6pt + accent)

#relationship-notice(accent)

#block(fill: luma(244), inset: (x: 8pt, y: 6pt), width: 100%)[
  #grid(
    columns: (1.4fr, 1fr, 1fr, 1.2fr), gutter: 8pt,
    strip-cell("Patient", required(subject.name)),
    strip-cell("Born / sex", required(subject.birthDate) + " · " + or-unknown(subject.sex)),
    strip-cell("Accession", required(specimen.accession)),
    strip-cell("Specimen", required(specimen.matrix) + " · " + containers),
  )
  #for identifier in list-of(subject.identifiers) [
    #text(size: 7pt)[#required(identifier.type) #required(identifier.value)
      (assigned by #required(identifier.authority))]
  ]
]

#grid(
  columns: (1fr, 1fr, 1fr), gutter: 12pt,
  [
    == Order
    #facts(order-facts, label-fill: muted, gutter: 3pt)
  ],
  [
    == Collection
    #facts(specimen-facts.slice(3), label-fill: muted, gutter: 3pt)
  ],
  [
    == Other
    #facts(
      (
        ("Medications", medications-text),
        ("Not reported", other-tests-text),
        ("Validity", validity-text),
      ),
      label-fill: muted,
      gutter: 3pt,
    )
  ],
)

#if result-groups.len() == 0 [
  == Results
  No results are reported in this revision.
]
#for group in result-groups [
  == Results — #required(group.name)
  #text(size: 7pt, fill: muted)[Method #required(group.method) · Certainty
    #required(group.certainty) · Role #required(group.role)]
  #v(2pt)
  #table(
    columns: (1.8fr, 0.8fr, 0.6fr, 1.3fr, 1.4fr, 1.2fr),
    stroke: (x, y) => (bottom: 0.4pt + rule),
    fill: (x, y) => if y == 0 { accent },
    inset: (x: 4pt, y: 4pt),
    align: (left, right, left, left, left, left),
    table.header(
      ..("Analyte", "Result", "Units", "Limits", "Comparison", "Recorded").map(label =>
        text(size: 7pt, weight: "bold", fill: white, label)),
    ),
    ..group.rows
      .map(row => (
        text(weight: "bold", required(row.analyte)),
        text(weight: "bold", required(row.result)),
        s(row.units),
        required(row.limits),
        comparison-label(row, size: 7pt),
        text(size: 7pt, fill: muted, or-unknown(optional-key(row, "recordedAt"))),
      ))
      .flatten(),
  )
]

#block(stroke: 0.6pt + rule, inset: 7pt, width: 100%, above: 10pt)[
  #set text(size: 7pt)
  #if result-notes.len() > 0 [
    *Result notes.* #result-notes.join(" ")
    #parbreak()
  ]
  *Preparation.* Prepared for release by #required(data.releasedBy.name),
  generated #generated-at. #preparation-statement
]

#align(center, text(size: 7pt, fill: muted)[— End of report —])
