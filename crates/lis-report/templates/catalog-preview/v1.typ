// Catalog Preview, template "catalog-preview" version 1 (proposed REQ-OPS-015).
//
// Issued templates are immutable: change the layout or the data contract in
// a new version file, never here (REQ-RPT-007).
//
// A laboratory manager checks every value of the test catalog on paper before
// approving it. The template is presentational: it lists what the engine
// supplies, in the order below, and adds no interpretation. Values are shown
// exactly as given; numbers are lexical strings and are never reformatted.
// Blank values and starter values to confirm are marked with words (BLANK,
// CONFIRM, DEMO) in a box, so a monochrome print keeps them visible.
//
// Inputs
// - sys.inputs.issued-at: supplied by the renderer to every template and not
//   used here. The printed generation times are report.generatedAt and
//   report.generatedAtUtc, already formatted by the engine.
// - /data.json: one JSON object. Every scalar is a string, shown exactly as
//   given; JSON numbers and booleans are refused before rendering. `null` is
//   allowed only where marked "or null".
//
//   report:         { id, generatedAt (laboratory time zone), generatedAtUtc,
//                     generatedBy, contentDigest }
//   laboratoryName
//   disclaimer      (or null)
//   gaps:           [{ record, analyte (or null),
//                      gap: "blank_cutoff" | "starter_threshold_rule"
//                         | "starter_threshold_rounding" }]
//   drafts:         [{ recordType: "validity_policy" | "method" | "test"
//                         | "panel", code, name, version, revision }]
//   specimenTypes:  [{ code, name }]
//   analytes:       [{ code, name, loincCode (or null),
//                      loincDisplayName (or null), drugClass (or null) }]
//   methods:        [{ code, name, versions: [{ version,
//                      status: "draft" | "in_effect", specimenType,
//                      certainty: "presumptive" | "definitive",
//                      outputScale: "qualitative" | "quantitative",
//                      thresholdRule: { value: "at_or_above" | "above",
//                                       source },
//                      thresholdRounding: { value: "before_rounding"
//                                       | "after_rounding", source },
//                      analytes: [{ code, name, unit (or null),
//                         cutoff (or null), cutoffSource, lod (or null),
//                         loq (or null), reportableLow (or null),
//                         reportableHigh (or null),
//                         displayScale (or null) }] }] }]
//   orderables:     [{ code, name, kind: "test" | "panel",
//                      versions: [{ version, status,
//                        components: [{ position, methodCode, methodVersion,
//                           purpose: "analytical" | "validity",
//                           analyteCodes: [string] }] }] }]
//   validityPolicies: [{ specimenType, versions: [{ version, status,
//                      categories: [{ code, label,
//                         action: "none" | "qualifier" | "hold" | "recollect"
//                            | "comment", qualifierText (or null) }] }] }]
//   loincNotice     (or null)
//
//   A source is { kind: "published", title, label } or { kind } with kind one
//   of "starter_value", "blank", "demo_value", "changed_by_laboratory",
//   "entered_by_laboratory".
//
// Rules
// - `cutoff` is null exactly when its `cutoffSource` is "blank".
// - Every required string is non-empty; an unknown code fails the render.

#let data = json("/data.json")

#let status-labels = (draft: "Draft", in_effect: "In effect")
#let kind-labels = (test: "Test", panel: "Panel")
#let certainty-labels = (presumptive: "Presumptive", definitive: "Definitive")
#let scale-labels = (qualitative: "Qualitative", quantitative: "Quantitative")
#let rule-labels = (at_or_above: "At or above", above: "Above")
#let rounding-labels = (
  before_rounding: "Before rounding",
  after_rounding: "After rounding",
)
#let purpose-labels = (analytical: "Analytical", validity: "Validity")
#let action-labels = (
  "none": "None",
  qualifier: "Qualifier",
  hold: "Hold",
  recollect: "Recollect",
  comment: "Comment",
)
#let draft-type-labels = (
  validity_policy: "Validity policy",
  method: "Method",
  test: "Test",
  panel: "Panel",
)
#let gap-labels = (
  blank_cutoff: "Cutoff is blank: laboratory to supply",
  starter_threshold_rule: "Threshold rule is a starter value: confirm",
  starter_threshold_rounding: "Threshold rounding is a starter value: confirm",
)

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
#let list-of(value) = {
  assert(type(value) == array, message: "contract: expected a list")
  value
}
// A string, or null shown as a dash.
#let or-dash(value) = if value == none { "–" } else { required(value) }
// A limit or range value, or null shown as the word "blank".
#let or-blank(value) = if value == none { emph("blank") } else { required(value) }
// The word that marks a value needing attention, boxed so it survives a
// monochrome print.
#let marker(word) = box(
  stroke: 0.8pt,
  fill: luma(220),
  inset: (x: 3pt, y: 1.5pt),
  outset: (y: 1pt),
  text(weight: "bold", word),
)

#let source-label(source) = {
  let kind = source.kind
  if kind == "published" [Published: #required(source.title) (#required(source.label))]
  else if kind == "starter_value" [#marker("CONFIRM") Starter value: confirm]
  else if kind == "blank" [#marker("BLANK") Blank: laboratory to supply]
  else if kind == "demo_value" [#marker("DEMO") Demo value: not a recommended cutoff]
  else if kind == "changed_by_laboratory" [Changed by the laboratory]
  else if kind == "entered_by_laboratory" [Entered by the laboratory]
  else { assert(false, message: "contract: unknown source kind") }
}

// A table that breaks across pages and repeats its header on each page.
#let listing(columns, headers, rows) = table(
  columns: columns,
  table.header(..headers.map(h => [*#h*])),
  ..rows.flatten(),
)

#let version-heading(title, version) = [
  === #title · version #required(version.version) (#status-labels.at(version.status))
]

#let report = data.report
// The digest is copied by hand, so it is never broken with spaces.
#let digest = {
  assert(type(report.contentDigest) == str and report.contentDigest != "",
    message: "contract: content digest is required")
  report.contentDigest
}
#let lab-name = required(data.laboratoryName)
#let gaps = list-of(data.gaps)
#let drafts = list-of(data.drafts)
#let methods = list-of(data.methods)
#let orderables = list-of(data.orderables)
#let validity-policies = list-of(data.validityPolicies)
#let count-versions(records) = records.map(record => list-of(record.versions).len()).sum(default: 0)

#set document(title: "Catalog Preview")
#set text(lang: "en", region: "US", size: 9pt)
#set par(justify: false)
#set table(
  inset: 3.5pt,
  stroke: 0.5pt + luma(120),
  fill: (_, y) => if y == 0 { luma(235) },
)
#set page(
  paper: "us-letter",
  flipped: true,
  margin: (x: 14mm, top: 22mm, bottom: 18mm),
  header: [
    #set text(size: 8.5pt)
    #lab-name · Catalog Preview #h(1fr)
    Generated #required(report.generatedAt) (#required(report.generatedAtUtc))
  ],
  footer: context [
    #set text(size: 8.5pt)
    Report #required(report.id) #h(1fr)
    Page #counter(page).display("1 of 1", both: true)
  ],
)

= Catalog Preview

#if data.disclaimer != none [
  #block(stroke: 2pt, inset: 8pt, width: 100%)[
    #text(size: 11pt, weight: "bold")[#required(data.disclaimer)]
  ]
]

/ Laboratory: #lab-name
/ Report: #required(report.id)
/ Generated: #required(report.generatedAt) (#required(report.generatedAtUtc))
/ Generated by: #required(report.generatedBy)
/ Content digest: #text(font: "DejaVu Sans Mono", size: 8pt)[#digest]

== Counts

#table(
  columns: (16em, 4em),
  table.header([*Record*], [*Count*]),
  [Specimen types], [#list-of(data.specimenTypes).len()],
  [Analytes], [#list-of(data.analytes).len()],
  [Methods], [#methods.len()],
  [Method versions], [#count-versions(methods)],
  [Tests], [#orderables.filter(o => o.kind == "test").len()],
  [Panels], [#orderables.filter(o => o.kind == "panel").len()],
  [Test and panel versions], [#count-versions(orderables)],
  [Validity policy versions], [#count-versions(validity-policies)],
  [Gaps: blank cutoffs], [#gaps.filter(g => g.gap == "blank_cutoff").len()],
  [Gaps: starter values to confirm], [#gaps.filter(g => g.gap != "blank_cutoff").len()],
  [Drafts awaiting review], [#drafts.len()],
)

== Gaps to resolve

#if gaps.len() == 0 [
  None.
] else [
  #listing(
    (2fr, 1.5fr, 3fr),
    ("Record", "Analyte", "Missing"),
    gaps.map(gap => (
      [#required(gap.record)],
      [#or-dash(gap.analyte)],
      [#marker(if gap.gap == "blank_cutoff" { "BLANK" } else { "CONFIRM" })
        #gap-labels.at(gap.gap)],
    )),
  )
]

== Drafts awaiting review

#if drafts.len() == 0 [
  None.
] else [
  #listing(
    (1.2fr, 1.5fr, 3fr, 0.8fr, 0.8fr),
    ("Record type", "Code", "Name", "Version", "Revision"),
    drafts.map(draft => (
      [#draft-type-labels.at(draft.recordType)],
      [#required(draft.code)],
      [#required(draft.name)],
      [#required(draft.version)],
      [#required(draft.revision)],
    )),
  )
]

== Specimen types

#if list-of(data.specimenTypes).len() == 0 [
  None.
] else [
  #listing(
    (1fr, 3fr),
    ("Code", "Name"),
    data.specimenTypes.map(t => ([#required(t.code)], [#required(t.name)])),
  )
]

== Analytes

#if list-of(data.analytes).len() == 0 [
  None.
] else [
  #listing(
    (1.4fr, 2.4fr, 1fr, 3fr, 1.4fr),
    ("Code", "Name", "LOINC code", "LOINC display name", "Drug class"),
    data.analytes.map(a => (
      [#required(a.code)],
      [#required(a.name)],
      [#or-dash(a.loincCode)],
      [#or-dash(a.loincDisplayName)],
      [#or-dash(a.drugClass)],
    )),
  )
]

== Methods

#if methods.len() == 0 [
  None.
]
#for method in methods [
  #for version in list-of(method.versions) [
    #version-heading([#required(method.code) #required(method.name)], version)
    #grid(
      columns: (auto, 1fr),
      column-gutter: 8pt,
      row-gutter: 4pt,
      [*Specimen type*], [#required(version.specimenType)],
      [*Certainty*], [#certainty-labels.at(version.certainty)],
      [*Output scale*], [#scale-labels.at(version.outputScale)],
      [*Threshold rule*],
      [#rule-labels.at(version.thresholdRule.value) ·
        #source-label(version.thresholdRule.source)],
      [*Threshold rounding*],
      [#rounding-labels.at(version.thresholdRounding.value) ·
        #source-label(version.thresholdRounding.source)],
    )
    #v(4pt)
    #listing(
      (1.3fr, 2fr, 0.8fr, 0.9fr, 2.6fr, 0.8fr, 0.8fr, 0.9fr, 0.9fr, 0.8fr),
      (
        "Analyte code", "Name", "Unit", "Cutoff", "Cutoff source", "LOD",
        "LOQ", "Reportable low", "Reportable high", "Display scale",
      ),
      list-of(version.analytes).map(row => {
        assert(
          (row.cutoff == none) == (row.cutoffSource.kind == "blank"),
          message: "contract: cutoff is null exactly when its source is blank",
        )
        (
          [#required(row.code)],
          [#required(row.name)],
          [#or-dash(row.unit)],
          [#if row.cutoff == none { marker("BLANK") } else { required(row.cutoff) }],
          [#source-label(row.cutoffSource)],
          [#or-blank(row.lod)],
          [#or-blank(row.loq)],
          [#or-blank(row.reportableLow)],
          [#or-blank(row.reportableHigh)],
          [#or-blank(row.displayScale)],
        )
      }),
    )
  ]
]

== Tests and panels

#if orderables.len() == 0 [
  None.
]
#for orderable in orderables [
  #for version in list-of(orderable.versions) [
    #version-heading(
      [#kind-labels.at(orderable.kind) #required(orderable.code) #required(orderable.name)],
      version,
    )
    #listing(
      (0.6fr, 1.6fr, 0.8fr, 1fr, 4fr),
      ("Position", "Method code", "Method version", "Purpose", "Analyte codes"),
      list-of(version.components).map(component => (
        [#required(component.position)],
        [#required(component.methodCode)],
        [#required(component.methodVersion)],
        [#purpose-labels.at(component.purpose)],
        [#list-of(component.analyteCodes).map(required).join(", ")],
      )),
    )
  ]
]

== Validity policies

#if validity-policies.len() == 0 [
  None.
]
#for policy in validity-policies [
  #for version in list-of(policy.versions) [
    #version-heading([Specimen type #required(policy.specimenType)], version)
    #listing(
      (1.2fr, 2fr, 1fr, 3fr),
      ("Category code", "Label", "Action", "Qualifier text"),
      list-of(version.categories).map(category => (
        [#required(category.code)],
        [#required(category.label)],
        [#action-labels.at(category.action)],
        [#or-dash(category.qualifierText)],
      )),
    )
  ]
]

== Source labels

Every value in this report that comes from the starter catalog or from the
laboratory carries one of these labels.

#table(
  columns: (1.4fr, 4fr),
  table.header([*Label*], [*Meaning*]),
  [Published: _source title_ (_label_)],
  [The value is stated in the named published source.],
  [#marker("CONFIRM") Starter value: confirm],
  [The product supplies a starting value the laboratory must confirm.],
  [#marker("BLANK") Blank: laboratory to supply],
  [No source gives this value; the laboratory must supply it or mark it as not applicable.],
  [#marker("DEMO") Demo value: not a recommended cutoff],
  [A fictitious value for the demo laboratory only.],
  [Changed by the laboratory],
  [The laboratory changed a supplied value.],
  [Entered by the laboratory],
  [The laboratory entered this value itself.],
)

#if data.loincNotice != none [
  == LOINC notice

  #required(data.loincNotice)
]
