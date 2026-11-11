
// Layout "Standard", version 1 (follows the shared contract above): a header
// band with the laboratory and the report number; Patient, Order and Specimen
// in three cards; a results table per ordered component; result notes; then
// medications, tests not reported, specimen validity and preparation.

#let accent = rgb("#163a5f")
#let muted = luma(95)
#let rule = luma(205)

#show: layout-page.with(accent)
#show heading.where(level: 1): set text(size: 15pt, fill: accent)
#show heading.where(level: 2): it => block(above: 12pt, below: 6pt, width: 100%)[
  #set text(size: 8pt, weight: "bold", fill: accent, tracking: 0.6pt)
  #upper(it.body)
  #v(-5pt)
  #line(length: 100%, stroke: 0.9pt + accent)
]
#show heading.where(level: 3): set text(size: 9.5pt, fill: accent)

#let pill(body) = box(
  fill: rgb("#e6edf5"), radius: 3pt, inset: (x: 6pt, y: 3pt),
  text(size: 7pt, weight: "bold", fill: accent, upper(body)),
)
#let card(title, pairs) = block(
  stroke: 0.6pt + rule, radius: 4pt, inset: 8pt, width: 100%, height: auto,
)[
  #heading(level: 2, outlined: false)[#title]
  #facts(pairs, label-fill: muted)
]

#block(fill: accent, radius: 4pt, inset: (x: 12pt, y: 10pt), width: 100%)[
  #set text(fill: white)
  #grid(
    columns: (1fr, auto), gutter: 12pt,
    [
      #text(size: 16pt, weight: "bold", required(lab.name)) \
      #text(size: 7.5pt, lab-line)
    ],
    align(right)[
      #text(size: 8pt)[Report] \
      #text(size: 13pt, weight: "bold", required(report.number)) \
      #text(size: 8pt)[Revision #required(report.revision)]
    ],
  )
]

#grid(
  columns: (1fr, auto), gutter: 8pt, align: horizon,
  heading(level: 1)[Clinical Toxicology Report],
  [#pill(completeness-label) #h(4pt) #pill(relationship-label)],
)
#text(size: 7.5pt, fill: muted)[Generated #generated-at]

#relationship-notice(accent)

#grid(
  columns: (1fr, 1fr, 1fr), gutter: 8pt,
  card("Patient", subject-facts),
  card("Order", order-facts),
  card("Specimen", specimen-facts),
)

== Results

#if result-groups.len() == 0 [
  No results are reported in this revision.
]
#for group in result-groups [
  #block(fill: rgb("#eef3f8"), inset: (x: 8pt, y: 6pt), width: 100%, below: 0pt)[
    #grid(
      columns: (1fr, auto), gutter: 8pt, align: horizon,
      heading(level: 3, required(group.name)),
      text(size: 8pt)[Method #required(group.method) · #required(group.certainty) ·
        #required(group.role)],
    )
  ]
  #table(
    columns: (2.2fr, 0.9fr, 0.7fr, 1.4fr, 1.4fr),
    stroke: (x, y) => (bottom: 0.4pt + rule),
    inset: (x: 5pt, y: 5pt),
    align: (left, right, left, left, left),
    table.header(
      ..("Analyte", "Result", "Units", "Limits", "Comparison").map(label =>
        text(size: 7pt, weight: "bold", fill: muted, tracking: 0.4pt, upper(label))),
    ),
    ..group.rows
      .map(row => (
        [#text(weight: "bold", required(row.analyte)) \
         #text(size: 7pt, fill: muted)[Recorded #or-unknown(optional-key(row, "recordedAt"))]],
        text(size: 10pt, weight: "bold", required(row.result)),
        s(row.units),
        required(row.limits),
        comparison-label(row),
      ))
      .flatten(),
  )
]

#if result-notes.len() > 0 [
  == Result notes
  #set text(size: 7.5pt)
  #for note in result-notes [
    - #note
  ]
]

#grid(
  columns: (1fr, 1fr), gutter: 16pt,
  [
    == Medications as supplied
    #if medication-lines.len() == 0 [None supplied.] else {
      for line in medication-lines [- #line]
    }
  ],
  [
    == Tests not reported
    #if other-test-lines.len() == 0 [None.] else {
      for line in other-test-lines [- #line]
    }
  ],
  [
    == Specimen validity
    #facts((("Outcome", required(validity.outcome)),), label-fill: muted)
    #for note in validity-notes [- #note]
  ],
  [
    == Preparation
    #facts(
      (
        ("Prepared for release by", required(data.releasedBy.name)),
        ("Generated", generated-at),
      ),
      label-fill: muted,
    )
    #text(size: 7pt, fill: muted, preparation-statement)
  ],
)
