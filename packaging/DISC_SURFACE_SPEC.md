# PLATTYPUS MGS — SCREEN-PRINTED DISC SURFACE ARTWORK
## PlayStation® Compact Disc Mastering & Silk-Screen Specifications

---

## 1. PHYSICAL DISC METRICS (ECMA-130 RED BOOK CD STANDARD)
* **Outer Disc Diameter**: $120.0\,\text{mm} \pm 0.3\,\text{mm}$
* **Center Spindle Hole**: $15.0\,\text{mm} \pm 0.1\,\text{mm}$
* **Mirror Band / Stacking Ring**: $46.0\,\text{mm}$ inner non-printable boundary.
* **Effective Printable Top Surface Area**: Outer $\varnothing 118.0\,\text{mm}$ to Inner $\varnothing 48.0\,\text{mm}$.
* **Print Process**: 3-Color Silk-Screen (Silver Base, Matte Black, Fluorescent Radar Green).

---

## 2. DISC FACE GRAPHIC LAYOUT
```
                 =================================
             ///                                   \\\
          ///     [COMPACT DISC DIGITAL AUDIO]        \\\
        //                                               \\
       /   [OFFICIAL SONY PLAYSTATION SEAL]                \
      /                                                     \
     |    PLATTYPUS                                          |
     |    TACTICAL ESPIONAGE ACTION                       |
     |                                                       |
     |                   +-------+                           |
     |                   | ( o ) |  <- CENTER SPINDLE        |
     |                   +-------+                           |
     |                                                       |
     |    SERIAL: BASLUS-00001                               |
      \   REGION: NTSC-U/C & PAL COMPLIANT                  /
       \  MADE IN AUSTRALIA / BURROW COMMAND               /
        \\                                               //
          \\\     ESRB: EVERYONE 10+                  ///
             \\\                                   ///
                 =================================
```

---

## 3. REQUIRED LEGAL & TECHNICAL NOTICES
1. **Compact Disc Digital Audio Logo**:
   * Standard Philips / Sony Red Book audio CD trademark placed at the 12 o'clock position.
2. **Official PlayStation® Trademark**:
   * Classic PlayStation logo positioned prominently with trademark registration symbol.
3. **Product Serial Identification**:
   * `BASLUS-00001` (NTSC-U/C)
   * `SLES-00001` (PAL / Australasia)
   * One codex, not one disc that boots everywhere. Real PlayStation hardware is
     region-locked and the two masters are separate images.
4. **Copyright & Rights Line**:
   * `© 2026 Burrow Command Studios. Licensed GPL-2.0-or-later.`
   * `Independent homebrew. Not affiliated with or endorsed by Sony Computer`
     ` Entertainment Inc. Full source in the project repository.`
5. **Red Book Audio Track Map**:
   * Track 1: MODE2/2352 data -- ISO 9660 image carrying the game executable,
     assets, and MDEC video streams (from 00:00:00).
   * Track 2: AUDIO -- 44.1 kHz 16-bit stereo CD-DA title soundtrack, with a
     2-second (150-sector) pregap between INDEX 00 and INDEX 01.
     **Do not hand-edit the MSF values.** Read them from `dist/plattypus.cue`
     after `make disc`; a mismatch means the printed art and the pressed master
     disagree. (`01:29:68` was previously quoted here and is not even a legal
     MSF value -- frames run 0..74.)
