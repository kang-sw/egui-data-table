---
title: "Clipboard source precedence between the system clipboard and internal state"
related-mental-model:
  - clipboard-tsv
---

# Clipboard source precedence between the system clipboard and internal state

## Question

When both the system clipboard and the internal clipboard hold content, which one
should a paste use — and how does the table know which is newer?

## Why it is open

The crate keeps its own clipboard (typed rows, exact fidelity) alongside the
system clipboard (TSV text, lossy but interoperable). A paste currently prefers
one of them by a fixed rule, and the code itself flags this as unresolved: there
is an acknowledged TODO about choosing between the system and internal source.

The consequence is ordinary and user-visible. Copy rows in the table, copy
something else in another application, paste back into the table: whether you get
the other application's text or your own earlier rows is decided by a rule the
user cannot see and did not choose. egui exposes clipboard *text*, not clipboard
*timestamps* or ownership, so "newer wins" is not directly implementable — which
is why this is research and not a bug.

## What is known

- The TSV codec is the interop surface; the internal clipboard is the fidelity
  surface. They are not interchangeable: a row that round-trips through TSV loses
  any field the viewer does not render as text.
- The system-decode path returns a `bool` that the caller discards, so a failed
  or empty decode is indistinguishable from a successful one at the call site.
  Whatever precedence rule is chosen, that signal needs to become load-bearing.
- The TSV encoder writes a single space for an empty cell, which diverges from
  the design note at the top of the clipboard module. Any precedence work has to
  decide whether that is the intended wire format or a bug, because it affects
  whether externally-produced TSV and internally-produced TSV compare equal —
  which is one of the few available heuristics for "did the system clipboard
  change since we last copied".

## Directions worth evaluating

- **Fingerprint the last copy.** Remember the TSV this table wrote; if the system
  clipboard still matches it, prefer the internal clipboard, otherwise prefer the
  system one. Needs the empty-cell divergence resolved first.
- **Always prefer the system clipboard, decode-or-fall-back.** Simplest and
  matches user expectation from other applications; costs fidelity on
  table-to-table paste, which is the crate's own main use case.
- **Make it the consumer's decision.** A `Style` or builder knob. Honest, but
  pushes an unresolved question onto every implementer.

## Exit criteria

A decision recorded with its rationale, the discarded `bool` given a purpose, and
the empty-cell TSV divergence settled one way or the other. Producing a follow-up
implementation ticket is the expected outcome; changing behavior is not part of
this ticket.
