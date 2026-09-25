# Where the numbers come from

Orchestre synthesizes every sound, so its character lives in a few hundred
design numbers. This page lists the ones taken from published sources, and
the ones that are still tuned by ear. When you change a sourced value, keep
this page in step.

## Sound effects

### Voice generator (`crates/dsp/src/sfx.rs`, `glottis.rs`)

| What | Value | Source |
|---|---|---|
| Vowel formants F1–F3 (oo, ee, ah, ae) | 300/870/2240, 270/2290/3010, 730/1090/2440, 660/1720/2410 Hz | Peterson & Barney 1952, men (checked against the `pb52` dataset, [phonTools](https://search.r-project.org/CRAN/refmans/phonTools/html/pb52.html)) |
| F4, F5 | 3300, 3750 Hz | Klatt 1980, Table I ([PDF](https://www.fon.hum.uva.nl/david/ma_ssp/doc/Klatt-1980-JAS000971.pdf)) |
| Formant bandwidths | 50·(1 + f²/6·10⁶) above 500 Hz | Tappert–Martony–Fant, corrected by Khodai-Joopari & Clermont 2002, as in [soundgen](https://cran.r-project.org/package=soundgen) |
| Glottal source | LF model from Rd (Ra, Rk, Rg equations) | Fant 1995; restated in COVAREP's [`Rd2tetpta.m`](https://github.com/covarep/covarep/blob/master/glottalsource/glottal_models/Rd2tetpta.m) |
| Formant scaling with throat size | all formants ∝ 1/length | uniform tube; Fitch 1997 (formant dispersion) |
| Scream flutter ("rasp") | 30–150 Hz amplitude modulation | Arnal et al. 2015, [Current Biology](https://www.cell.com/fulltext/S0960-9822(15)00737-X) |
| Jitter/shimmer at full roughness | ±5% / ±35% (5–10× pathology thresholds) | [Praat manual](https://www.fon.hum.uva.nl/praat/manual/Voice_2__Jitter.html); Anikin 2020 |
| Bandwidth scaling √size | — | **by ear** (direction matches Kent & Vorperian) |

### Presets (`crates/core/src/sfx/presets.rs`)

Each preset built from a source says so in a comment. The main ones:

| Preset | Source |
|---|---|
| Scream | soundgen scream preset; Pisanski et al. 2020 |
| Laugh | Bachorowski, Smoski & Owren 2001 |
| Dog growl | Faragó et al. 2010 |
| Dog bark | **by ear**: a rough woof over a chest voice. Real barks (Yin 2002; Pongrácz et al.; Malinois formants in [Vet. Sci. 2026](https://pmc.ncbi.nlm.nih.gov/articles/PMC13307902/)) come from a throat this formant model can't reproduce convincingly; the *Dog* throat option (those formants) works better for the duck quack |
| Cat meow | Schötz et al. 2024 |
| Cow moo | Padilla de la Torre et al. 2015 (throat size by ear) |
| Horse neigh | Briefer et al. 2015 (two pitches ~4× apart; wobble by ear) |
| Wolf howl | Kershenbaum et al. 2018 |
| Owl hoot | great horned owl, Ardea 97(4) |
| Mouse squeak, bird chirp, frog, insects | search-summary figures (lower confidence) |
| Monster roar | lion: Klemuk et al. 2011, Eklund et al. 2011 |
| Door creak / open | Farnell's creaking door, followed closely: clean stick-slip clicks (1–333/s, louder the longer stuck), wood resonances Q 1–3, panel delays 4.52–16 ms, HPF 125 Hz; *Designing Sound*, [creaking door](https://en.wikibooks.org/wiki/Designing_Sound_in_SuperCollider/Creaking_door) |
| Fire, bonfire | Farnell, [fire](https://en.wikibooks.org/wiki/Designing_Sound_in_SuperCollider/Fire) |
| Thunder | Fineberg, Walters & Reiss 2022, [arXiv](https://arxiv.org/html/2204.08026) |
| Bubbles generator; splash, stream, swim, drips | van den Doel 2005, [PDF](http://www.cs.ubc.ca/~kvdoel/publications/tap05.pdf): f ≈ 3/r, decay 0.043f + 0.0014f^1.5, pitch rise ξ·d·t, radius power law |
| Gunshots | three-event model, Hacıhabiboğlu (Friedlander blast) |
| Engines | firing rate = rpm/60 × cylinders/2 (four-stroke); pulses into an exhaust waveguide whose resonance doesn't follow rpm; kept clean at speed as in Farnell's [car engines](https://en.wikibooks.org/wiki/Designing_Sound_in_SuperCollider/Cars) |
| Rain density | van den Doel 2005 (rates for light to heavy rain) |
| Coin | the classic B5→E6 recipe |

Still by ear: most gains and envelopes, wind, footsteps (heel/toe timing),
UI sounds, magic, sci-fi, and machines other than the engine's rpm.

### Materials (resonance generator)

| What | Value | Source |
|---|---|---|
| Metal / wood bar modes | 1, 2.756, 5.404, 8.933, 13.344, 18.638 | free-bar theory (Euler–Bernoulli) |
| Glass modes | 1, 2.83, 5.42, 8.77, 12.87, 17.71 | French's bowl theory ([arXiv:1106.5657](https://arxiv.org/pdf/1106.5657)); Tibetan bowl measurements in the Csound manual's modal ratio table agree |
| Beating pairs | glass 4 Hz | Jundt et al. 2006 |
| Dense modes (blades, plates, rungs) | extra irregular modes 8–43% apart above the material's six | plates have crowded, near-uniform mode density (Fletcher & Rossing); spacing **by ear** |
| Bell partials | hum, prime, tierce 1.2, quint, nominal, superquint | "true harmonic" tuning ([keltektrust](https://www.keltektrust.org.uk/sob04.html)) |
| Damping ∝ frequency | decay rate ∝ f | van den Doel's modal models; Aramaki et al. 2011 |
| Loss factors | glass 1e-3, oak 1e-2, steel 2e-4 | Irvine, [damping properties of materials](http://vibrationdata.com/tutorials_alt/damping.pdf); bell bronze **guessed** |

Note: the Csound manual's modal table lists a wine glass as 1, 2.32, 4.25,
6.63, 9.38 (unsourced, "caveat emptor"); the bowl theory and measured bowls
disagree with it, so we don't use it. It also lists a measured uniform
wooden bar (1, 2.572, 4.644, 6.984, 9.723, 12) that could replace the free
bar for wood.

## Instruments

| Instrument | What | Source |
|---|---|---|
| Piano | inharmonicity B = exp(0.0926·m − 13.64) (treble line) | Rigaud, David & Daudet, JASA 2013 |
| | decay anchored at E♭4: prompt T60 7.5 s, aftersound 4× slower | Weinreich 1977 |
| | unison detune ~1–2 cents | Kirk 1959 |
| | no dampers from G6 up | piano construction (top 18 keys) |
| | hammer at ~1/8 of the string | Conklin |
| E-piano | 1:1 carrier with a 14× tine | DX7 "E.PIANO 1" |
| | ring times per register | Shear 2011 (one 1974 Mark I) |
| Organ | drawbar harmonics | Hammond footages |
| | ~3 dB per drawbar step | [stefanv.com](https://www.stefanv.com/electronics/hammond_drawbar_science.html) |
| | fold-back above 5924.6 Hz | top tonewheel |
| | percussion: 1 s, single-trigger, takes the 1' drawbar | Hammond manuals; setBfree defaults |
| | Leslie speeds and inertia | setBfree defaults; horn Doppler ±0.5 ms (JOS) |
| | jazz 888000000, rock 888800000 | Jimmy Smith, Jon Lord; church = full organ (no documented registration) |
| Mallets | marimba 1:3.92:9.24, vibraphone 1:4:10, xylophone 1:3:6.16:10.29 | euphonics.org; vibraphone tuning |
| | ring times | US patent 4,411,187 (rough) |
| Plucked | Karplus–Strong structure | Jaffe & Smith 1983 |
| | ring time ∝ f^-0.4 | Woodhouse 2021 (harp) |
| | guitar body 105/230 Hz; koto 85/100 Hz | guitar acoustics; ICA 2019 koto measurements |
| | pluck points: guitar ~0.2 of the string, harp near the middle | Traube & Smith; Woodhouse |
| Drums | cymbal/hat oscillators 205.3–800 Hz, unscaled | Werner, Abel & Smith, "The TR-808 Cymbal" |
| | cowbell 540/800 Hz, band-pass 850 Hz Q 4.25 | Werner et al., "More Cowbell" |
| | snare, rim, claves, toms, congas | TR-808 service notes |
| | 808 kit untransposed (~50 Hz kick) | Werner et al. 2014 |
| Synth | supersaw: 7 voices, uneven spacing, centre/side mix | Szabo, JP-8000 analysis |
| | 808 bass starts ~1 semitone sharp | measurements of 37 TR-808 samples |
| | flute vibrato mostly brightness, ~5 Hz | Sound on Sound, "Practical Flute Synthesis" |
| | strings vibrato 6 Hz | string vibrato norms |
| | ensemble chorus: 3 taps, 0.6 Hz + 6 Hz | Solina/Haible (rates approximate) |
| | chiptune 25% pulse | NES/Game Boy duty cycles |
| Choir | formants F1–F5 per voice type | Csound manual, Appendix D |
| | vibrato ~5.8 Hz, up to ~±1 semitone | Sundberg; Prame |
| | ensemble spread | Ternström (within preferred range) |
| Reverb | Freeverb constants | Jezar's `tuning.h` (verified) |

Still by ear: piano partial decay slope, two-stage split and hammer noise;
e-piano bark envelope and tine ping; organ click and envelope, Leslie
loudness sweep; mallet mode levels; guitar/bass body Q; the lead, bass, pad,
pluck, bell and wobble synth presets.
