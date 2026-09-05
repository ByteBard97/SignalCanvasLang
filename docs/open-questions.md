---
layout: default
title: Open Questions
permalink: /open-questions/
---

# Open Questions

Spec questions that were raised during v0.2 development. All were resolved through Socratic debates (full rationale in the Appendix).

**Q1: Does the `card` keyword approach work, or cards defined inside templates?**

No `card` keyword. Cards are regular templates with `meta { kind: "card", fits: "MY_Format" }`. Cards and templates are structurally identical: both have `ports` and `meta`. If we later need a `card` keyword, we can add it as syntactic sugar that desugars to a template (non-breaking). Removing a keyword would be breaking. See Compiler section → "Cards Are Templates."

**Q2: Should `ring` member accept just instance name or require port reference?**

Both. The parser accepts implicit (`member Console`) and explicit (`member Console.OptoCore_A`) forms. The emitter **must always output the explicit form**: implicit breaks if someone later adds a second ring port to a device. See Compiler section → "Ring Networks (Detailed)."

**Q3: Concerns about the slot block extension?**

No concerns: the slot block extension (`slot MY_Slot[1..3]: MY_Format { direction: "any", channels: 16 }`) is supported. But the `compatible` list proposed in the original doc was **inverted**: cards declare `fits: "MY_Format"` in their meta instead of slots listing compatible cards. This avoids editing every template when a new card type ships. See Compiler section → "Inverted Slot Compatibility."

**Q4: Reserve `card` and `ring` keywords now?**

Reserve `ring` and `member`: yes (concrete grammar design exists). Do NOT reserve `card`: we decided against the keyword, so reserving it would be misleading. `card` remains a valid identifier. See Appendix → "Reserved Keywords."

**Q5: Preference for deterministic ID scheme?**

Double-colon separator: `pl::CL5::Dante_In`. The proposed `pl_templateName_portName` is ambiguous because identifiers contain underscores (is `pl_Dante_Patch_In` template `Dante_Patch` + port `In`, or template `Dante` + port `Patch_In`?). The `::` separator cannot appear in PatchLang identifiers, making parsing unambiguous. Route IDs: `rule::template::src::dst`. Slot IDs: `slot::template::slot`. The loader should accept both `pl_` (legacy) and `pl::` (new) during migration. See Compiler section → "Deterministic Port IDs."
