# Asset attribution

Every non-code asset in this repository (images, icons, fonts, example drawings, presets) is listed
here with its author, source and licence. `cargo xtask assets` (part of `cargo xtask ci`) fails if
an asset file is missing from this table.

**Policy (mandatory):** CADCraft contains **no Autodesk, Adobe or Avid iconography, images,
artwork, fonts, hatch patterns, linetypes, templates or presets.** Every asset is original work by
CADCraft contributors or third-party material under an open licence (OSI open source, public domain
/ CC0, or Creative Commons that allows redistribution). Screenshots of Autodesk software are never
committed. Font files are not added here: shared fonts live in
[storytold/craft-fonts](https://github.com/storytold/craft-fonts), an optional build input. The one
exception is the Redrob mark in `assets/brand/` and the app icons derived from it in
`assets/app-icon/`: Redrob's own brand, copied verbatim from the Redrob design system (pinned by
`assets/brand/SHA256SUMS`), not open source and not covered by the code licence.

A second exception is the Korean UI face `crates/ui-egui/assets/fonts/WantedSans-Regular.ttf`:
egui's built-in fonts carry no Hangul, so it is embedded in the binary. It is the Redrob design
system's Korean system face, copied verbatim, under the SIL Open Font License 1.1 (licence text
beside it).

Generated-in-code assets are original and have no file to list:
- the UI icon set (`crates/ui-egui/src/icons.rs`);
- the "CADCraft Stroke" single-stroke drafting font (`crates/fonts/src/stroke.rs`);
- the standard linetype and hatch-pattern libraries (`crates/doc/src/library.rs`; industry-common
  names, our own dash/spacing values);
- the colour index palette, generated from its structure (`crates/color/src/lib.rs`);
- the sample drawings (`crates/engine/src/sample.rs`).

| Asset | Author | Source | Licence | Notes |
|---|---|---|---|---|
| `crates/ui-egui/assets/fonts/WantedSans-Regular.ttf` | Wanted Lab, Inc. (The Wanted Sans Project Authors) | Redrob design system `assets/Fonts/`, from [wanteddev/wanted-sans](https://github.com/wanteddev/wanted-sans) v1.003 | SIL OFL 1.1 | Korean UI face, embedded |
| `crates/ui-egui/assets/fonts/WantedSans-OFL.txt` | The Wanted Sans Project Authors | [wanteddev/wanted-sans](https://github.com/wanteddev/wanted-sans) `OFL.txt` | SIL OFL 1.1 | Licence of the face above |
| `assets/app-icon/hicolor/128x128/apps/ai.redrob.cad.png` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/hicolor/16x16/apps/ai.redrob.cad.png` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/hicolor/24x24/apps/ai.redrob.cad.png` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/hicolor/256x256/apps/ai.redrob.cad.png` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/hicolor/32x32/apps/ai.redrob.cad.png` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/hicolor/48x48/apps/ai.redrob.cad.png` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/hicolor/512x512/apps/ai.redrob.cad.png` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/hicolor/64x64/apps/ai.redrob.cad.png` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/redrob-cad-256.png` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/redrob-cad-512.png` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/redrob-cad.icns` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/app-icon/redrob-cad.ico` | Redrob | derived from `assets/brand/` by `packaging/brand-assets.sh` | Redrob brand, all rights reserved | App icon |
| `assets/brand/README.md` | Redrob CAD contributors | written for this repository | MIT OR Apache-2.0 | Rules for the brand sources |
| `assets/brand/SHA256SUMS` | Redrob CAD contributors | written for this repository | MIT OR Apache-2.0 | Checksums of the brand sources |
| `assets/brand/favicon-16x16.png` | Redrob | redrob-design-system `assets/Favicons/favicon-16x16.png`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `assets/brand/favicon-32x32.png` | Redrob | redrob-design-system `assets/Favicons/favicon-32x32.png`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `assets/brand/favicon-48x48.png` | Redrob | redrob-design-system `assets/Favicons/favicon-48x48.png`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `assets/brand/favicon-64x64.png` | Redrob | redrob-design-system `assets/Favicons/favicon-64x64.png`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `assets/brand/favicon.ico` | Redrob | redrob-design-system `assets/Favicons/favicon.ico`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `assets/brand/icon-192.png` | Redrob | redrob-design-system `assets/Favicons/icon-192.png`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `assets/brand/icon-512.png` | Redrob | redrob-design-system `assets/Favicons/icon-512.png`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `assets/brand/symbol-white-192.png` | Redrob | redrob-design-system `assets/Favicons/symbol-white-192.png`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `assets/brand/symbol-white-32.png` | Redrob | redrob-design-system `assets/Favicons/symbol-white-32.png`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `assets/brand/symbol-white-512.png` | Redrob | redrob-design-system `assets/Favicons/symbol-white-512.png`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `assets/brand/symbol-white-64.png` | Redrob | redrob-design-system `assets/Favicons/symbol-white-64.png`, verbatim | Redrob brand, all rights reserved | Pinned by `assets/brand/SHA256SUMS` |
| `examples/apartment.dxf` | CADCraft contributors | sample drawing made with cadcraft-cli commands | MIT OR Apache-2.0 | Original |
| `docs/images/ui-apartment.png` | CADCraft contributors | screenshot of CADCraft itself (examples/apartment.dxf) | MIT OR Apache-2.0 | Original; no Autodesk UI |
| `docs/images/ui-layout.png` | CADCraft contributors | screenshot of CADCraft itself (a layout of examples/apartment.dxf) | MIT OR Apache-2.0 | Original; no Autodesk UI |
| `docs/images/ui-bracket.png` | CADCraft contributors | screenshot of CADCraft itself (sample drawing generated in code) | MIT OR Apache-2.0 | Original; no Autodesk UI |
