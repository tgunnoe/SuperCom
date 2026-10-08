<!-- spec-lint: --inventory {file} --min-engine-rows 3 -->

# Green: an inventory with engine and Lua rows

| Name            | Owner  | Citation                  | Notes           |
| --------------- | ------ | ------------------------- | --------------- |
| sample_method_a | engine | https://example.com/api#a | an extra column |
| sample_method_b | engine | https://example.com/api#b |                 |
| sample_method_c | lua    | https://example.com/api#c |                 |
| sample_method_d | engine | https://example.com/api#d | a \| in a cell  |

A table that lacks the Citation column is ignored:

| Name | Owner  | Status |
| ---- | ------ | ------ |
| sim  | design | WIP    |

Pipe lines without a delimiter row are not a table:

| Name | Owner | Citation |
| not | a | delimiter |
| bad | native | |

So is a table inside a fence, even a malformed one:

```text
| Name | Owner | Citation |
| --- | --- | --- |
| bad | native |  |
```
