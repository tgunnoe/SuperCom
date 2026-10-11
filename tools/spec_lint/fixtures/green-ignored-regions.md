# Green: regions that are not clauses

Fenced code is skipped, even when it holds something clause-shaped:

```text
**FM-001** this would be malformed outside a fence
**FMT-001** `bogus` `test:`
```

**FMT-001** `design` `source:https://example.com/notes`
A clause directly after a fence is still found.

An HTML comment is skipped too:

<!--
**FM-7** `commented out`
-->

**FMT-002** `design` `source:https://example.com/notes`
A clause directly after a comment is still found.

A quote or a list item may start with bold text:

> **Note** quoted text

- **Item** listed text

**FMT-003** `design` `source:https://example.com/notes`
A clause after quoted and listed bold text is still found.

Bold text in the middle of a line, like a **term**, is ordinary prose.
