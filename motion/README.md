# zcash-disclosure motion

A 33 second motion graphic for the site, in the site's style: the lit sign, Geist Sans, Mono and Pixel, the dark theme. Text and cards move on Apple-style springs. Small isometric micrographics fold between 2D and 3D. The soundtrack is the site's sound engine (`site/src/lib/buzz.ts` and `seq.ts`) laid out on a score.

`index.html` draws every frame from a single time value. `timeline.js` holds the score and the drawing, `sound.js` builds the audio graph against the same score, and `main.js` either plays the score live or waits for `render.mjs` to drive it. The fonts and the buzz recording are read from `../site/public/` by relative path, so the page has to be served from the repo root.

## Scenes

The tempo is 124 BPM, so a beat is 0.484 s and a bar is 1.935 s. Every cut and every move starts on a beat.

| # | Scene | Bars | Seconds | Cues |
|---|---|---|---|---|
| 1 | The sign: "Sapling note disclosures, decoded in your browser." | 0 to 4 | 0.00 to 7.74 | black until beat 2, then the sign powers on. "Sapling" faults at beats 6, 8, 9.75, 10.25, 13 and 14.25 |
| 2 | A shielded transaction hides the amount and the recipient | 4 to 7 | 7.74 to 13.55 | three output slabs lie flat, then fold into an isometric stack from beat 17. Second line at beat 21 |
| 3 | A disclosure is one string | 7 to 11 | 13.55 to 21.29 | the first lines of the disclosure grammar type from beat 30 to 37, the note on it being unsigned at beat 38 |
| 4 | What it reveals: a Sapling output | 11 to 14 | 21.29 to 27.10 | an isometric stack: the top slab lifts at beat 44.5 and lies flat at 45.25. The card at beat 46, the value, address and memo fields, each a blank bar, on beats 47, 48 and 49, the verify line at beat 50 |
| 5 | End card: the icon, the wordmark and the sign | 14 to 16.5 | 27.10 to 31.94 | a 3D wireframe globe with a 15 degree grid, its polar axis and an orbit spins in the middle for two beats, then settles face on as it glides into the icon. At beat 60.5 the tile and the sign cut in and the wordmark rises. "Sapling" drops out at beat 63 |
| | Black and silent | 16.5 to 17 | 31.94 to 32.90 | |

The drone and the buzz come up with the sign at beat 2. The sequencer plays from bar 4 to bar 14, through Am(add9), Am, Fmaj7 and E7(b9), one chord per bar. It is a Berlin-school sequence in the manner of Tangerine Dream and Tron: Legacy, not the site's arpeggio. A sawtooth bass pulses root and octave in sixteenths through a resonant filter that opens from 260 Hz to 1.6 kHz over the whole phrase. From bar 7 a square-wave line plays chord tones above it on a seven-step pattern, so it shifts against the bar, with a dotted-eighth echo thrown to the other side of the stereo field. The buzz sounds only while a sign is on screen, in scenes 1 and 5. Each fault on "Sapling" makes the master gain jump, as on the site. Picture and sound cut out together at bar 16.5. The site's sequencer, `site/src/lib/seq.ts`, plays the same sequence, with the filter opening over sixteen bars and closing over the next sixteen since it loops.

## Motion

Everything that moves runs on Apple's spring, set by a damping ratio and a response in seconds, solved in closed form so any frame can be drawn on its own. Text, cards and the code block rise 24 px into place on a critically damped spring (damping 1, response 0.45 s), siblings 60 ms apart. Each scene leaves upward half a beat before its cut (damping 1, response 0.3 s), so the film flows in one direction. The two sign scenes do not move: the sign switches on and off.

Only the micrographics go between 2D and 3D. Nothing has a perspective, so their 3D is orthographic, and they turn to the true isometric angles. Their position uses damping 1 and their rotation uses Apple's rotation spring, damping 0.8, response 0.5 s. The tracks are in `TRACKS` in `timeline.js`. A key that starts while the one before is still moving springs from where that one had got to.

The globe is drawn in the icon's own coordinates and projected each frame. It spins at 120 degrees a second, then hands that speed to the rotation spring (damping 0.8, response 0.7 s), so the settle starts without a stall. As it settles the grid, the axis and the orbit fade out. What is left at yaw 0 and tilt 0 falls exactly on the lines of `site/public/icon.svg` (a rim, a prime meridian, meridians 45 degrees either side, an equator and parallels at 30 degrees), which is why the cut to the icon at beat 60.5 does not jump. The strokes thin as the globe grows, so they keep one width on screen.

In the live preview, `prefers-reduced-motion: reduce` drops all movement and keeps a 200 ms fade.

The fault shapes are the three from `site/src/pages/Home.tsx` (stutter, dropout, sag), drawn from a seeded generator so every render is the same.

## Render

```sh
npm install
node render.mjs
```

This writes `out/zcash-disclosure.mp4` (1920x1080, 60 fps, H.264 and AAC) and the soundtrack alone as `out/zcash-disclosure.wav`. It takes about two minutes. The script serves the repo root on a local port, renders the audio in an `OfflineAudioContext` in the page, then screenshots the 1280x720 stage at a device scale of 1.5 for every frame and pipes the PNGs into ffmpeg.

It launches the newest `chromium_headless_shell-*` in `~/Library/Caches/ms-playwright/` with the Chromium sandbox off. Set `CHROMIUM` to use another binary.

ffmpeg needs libx264 and AAC. The script takes `FFMPEG` if it is set, then `motion/bin/ffmpeg` if that file exists, then `/opt/homebrew/bin/ffmpeg`, then `ffmpeg` on the PATH. A static build such as [ffmpeg-static b6.1.1](https://github.com/eugeneware/ffmpeg-static/releases/tag/b6.1.1) can go in `motion/bin/`, which git ignores. Another way is `npm install ffmpeg-static` in `motion/` and `FFMPEG` set to the path it exports. The ffmpeg in Playwright's cache only writes VP8 and WebM, so it cannot make the mp4.

`node render.mjs --still 4 16.9` writes single frames as `out/still-4.png` and `out/still-16.9.png`.

## Preview

```sh
node render.mjs --preview
```

This serves the page at `http://127.0.0.1:5180/motion/index.html`. Press Play to run the score live in the browser, with sound. Add `?t=16.9` to the URL to hold one frame.
