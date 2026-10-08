# Green: install locators with and without an archive member

**FMT-001** `game-data` `source:fa:bin/sample.lua`
A file under the install is a valid locator.

**FMT-002** `game-data` `source:fa:gamedata/sample.scd!units/sample/sample_unit.bp`
An archive member after the bang is a valid locator.

**FMT-003** `game-data` `source:fa:gamedata/sample.scd!units/sample.bp#L3`
An anchor after the locator is ignored.
