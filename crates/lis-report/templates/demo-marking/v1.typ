// Demo marking v1 (proposed REQ-OPS-016): the demo laboratory's watermark. It wraps any
// report template, which runs unchanged as `/template.typ`, and puts "DEMO" and a
// not-for-patient-testing line behind the content of every page. The document keywords
// say so too, in the PDF's plain metadata, for any tool that reads it. Never edit this
// file once issued: its digest is pinned. A changed marking is a new version.
#set document(keywords: ("DEMO", "Demo laboratory: not for patient testing"))
#set page(background: place(center + horizon, rotate(-40deg, align(center, stack(
  spacing: 12pt,
  text(size: 120pt, weight: "bold", fill: luma(88%))[DEMO],
  text(size: 18pt, weight: "bold", fill: luma(80%))[Demo laboratory: not for patient testing],
)))))
#include "/template.typ"
