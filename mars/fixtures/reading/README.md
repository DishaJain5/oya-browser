<!-- Provenance of frozen public compatibility inputs, not a browser conformance claim. -->
# Static reading fixtures

`example.html` and `httpbin.html` preserve the exact public response bodies observed
through Oya Native's owned network service on 2026-10-23 from
`https://example.com/` and `https://httpbin.org/html`. They are offline regression
inputs; tests do not contact either service. Preserve their bytes and resource
references rather than rewriting the page into the supported subset.

The httpbin paragraph is an excerpt from Herman Melville's public-domain
*Moby-Dick*. Its complete 3,588 UTF-8 bytes must remain one original text-bearing
node. The example input contains a required `/s.js` reference; stage-level CSS
tests keep it intact. Passing a stage-level test does not establish successful
navigation when that required script uses unsupported DOM/JavaScript features.
No original external script is bundled or silently ignored.

These data fixtures retain their original HTML instead of a repository source
header so the unchanged-source regression remains meaningful.
