---
layout: default
title: PatchLang
---

<p align="center">
  <img src="images/logo.png" alt="SignalCanvas" width="400">
</p>

# PatchLang

A domain-specific language for describing signal flow in broadcast and live production environments. Human-readable, git-diffable, LLM-friendly.

```patch
template Rio3224 {
  meta { manufacturer: "Yamaha"  model: "Rio3224"  category: "Stagebox" }
  ports {
    Dante_Pri: io(etherCON) [Dante, primary]
    Mic_In[1..32]: in(XLR)
    Line_Out[1..16]: out(XLR)
  }
  bridge Mic_In -> Dante_Pri
}

instance Stage_Left is Rio3224 { location: "Stage Left Wing" }
connect Stage_Left.Dante_Pri -> FOH.Dante_Pri { cable: "Cat6a"  length: "30m" }
bridge Stage_Left.Mic_In[1..32] -> FOH.Dante_Ch[1..32]
```

---

## Language

| | |
|---|---|
| [Overview](overview/) | What PatchLang is and why it exists |
| [Language Reference](language-reference/) | Full grammar and syntax reference |
| [Examples](examples/) | Real-world signal flow examples |
| [Changelog](changelog/) | Version history |

## Architecture

| | |
|---|---|
| [Project Structure](project-structure/) | How PatchLang files and projects are organized |
| [Compiler](compiler/) | Compiler internals, DRC rules, WASM/Python bindings |
| [Backend](backend/) | Django API integration |

## Design

| | |
|---|---|
| [Design Decisions](decisions/) | Recorded architectural decisions (D001-D018) |
| [Debate Context](debate-context/) | Structured debates behind key design choices |
| [Appendix](appendix/) | Reference tables and supplementary material |
| [Open Questions](open-questions/) | Spec questions raised during development and their resolutions |
