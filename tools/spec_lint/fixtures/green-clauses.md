# Green: well-formed clauses

Prose before the clauses may use **bold** and `code` freely.

**FMT-001** `game-data` `script:tools/survey/archive_census.py`
The archive layer MUST list every member.

**FMT-002** `reference` `source:https://example.com/format#header`
The header layout SHOULD follow the published description.
A second statement line continues the same paragraph.

**DAT-001** `engine-stub` `script:tools/survey/blueprint_census.py --quiet`
Arguments after the script path are not part of the path.

**SIM-001** `design` `source:fa:gamedata/sample.scd!units/sample`
An install locator naming an archive member is a valid source.

**DET-001** `derived` `script:tools/survey/format_versions.py`
A clause MAY follow from other clauses.

**Opt-001** `design` `source:https://example.com/notes`
A tag MAY use lower case letters.
