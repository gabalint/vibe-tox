// Preview marking v1 (REQ-RPT-007): the mark of a report layout preview. It wraps a
// report template, which runs unchanged as `/template.typ` on synthetic sample data,
// and prints "PREVIEW – NOT A REPORT" at the top of every page and faintly behind its
// content. The document keywords say so too, in the PDF's plain metadata. Never edit
// this file once issued: its digest is pinned. A changed marking is a new version.
#set document(keywords: ("PREVIEW", "Preview: not a report"))
#set page(background: {
  place(top + right, dx: -16mm, dy: 6mm, text(
    size: 7.5pt, weight: "bold", fill: rgb("#b42318"), tracking: 1pt,
  )[PREVIEW – NOT A REPORT])
  place(center + horizon, rotate(-40deg, text(
    size: 54pt, weight: "bold", fill: luma(93%),
  )[PREVIEW – NOT A REPORT]))
})
#include "/template.typ"
