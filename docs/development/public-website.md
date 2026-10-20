# Public website and presentation

The presentation is published at
[gabalint.github.io/vibe-tox](https://gabalint.github.io/vibe-tox/) from the public
repository [gabalint/vibe-tox](https://github.com/gabalint/vibe-tox); see
[publication](#publication).

The website summarizes the [license](../architecture/decisions/0017-gplv3-license.md) and links the
[development disclaimer](../governance/development-disclaimer.md). The video carries its own
synthetic-data and clinical-validation notices, and the page carries a vibe-coded disclaimer; keep
these notices current. Signed releases, hosted services and clinical readiness still require their
own approvals (Q-11, Q-19 in the [decision queue](../planning/open-questions.md)). The Kokoro and
soundfont notices in [media credits](../../website/media/credits.txt) retain their own terms.

## Publication

GitHub Pages uses **GitHub Actions** as its publishing source. Changes to `website/`
or `.github/workflows/pages.yml` on `main` trigger publication, and
`workflow_dispatch` supports a manual deployment. The workflow uploads the already
prepared static files, so neither the Rust engine nor the desktop UI must build to
publish the presentation. Permissions are limited to reading source and deploying
Pages with an OIDC token. Every merged website change is public within minutes;
review it in the local preview first. This guide does not attest to a current
remote-state check.

## Content and design

The canonical product copy is [the overview](../overview.md), checked against the
[implementation plan](../planning/implementation-plan.md) and
[operator guide](../operations/README.md). The intended readers are laboratory owners,
potential collaborators, and developers evaluating the project. The page moves from
the single-computer proposition to the video, the workflow shown through application
screens, data ownership, building from source, development status with the license and
disclaimer, and the repository. Its actions open the presentation, documentation,
contribution and security guidance, or source.

The website inherits the application's blue/neutral palette and system typography
from `ui/src/styles/tokens.css`, with larger type and spacing for reading. It uses
native tables and a video player, clear focus indicators, and responsive layouts.
The hero's workflow illustration describes real stages; it is not an application
screenshot. The workflow section shows six stills cropped from the supplied recording,
so they carry its limits: the real React interface with a mocked engine and synthetic
data. The note beneath them says so; keep it with the stills. The build-from-source
commands follow the [developer guide](README.md#build-and-run); update both together.
No analytics, contact form, account, clinical interface, or JavaScript runtime is used.

The site serves the supplied `vibe-tox-overview.mp4` and its poster. It is a 121.233-second
recording of the actual React interface with a mocked engine and synthetic data,
with narration and music. It demonstrates a workflow, not end-to-end clinical
execution or validation.

English captions, a descriptive transcript and audio credits accompany the video.
Keep development status and platform verification limits accurate when changing
the page or choosing a later recording.

## Files and updates

| File | Purpose |
| --- | --- |
| `website/index.html` | Public content, metadata, navigation, native player and links |
| `website/styles.css`, `website/responsive.css` | Presentation styles and responsive adaptations |
| `website/favicon.svg` | Authored geometric V mark |
| `website/media/vibe-tox-overview.mp4` | Supplied narrated H.264/AAC application walkthrough; progressive playback |
| `website/media/vibe-tox-overview-poster.jpg` | Supplied title poster, unchanged |
| `website/media/vibe-tox-overview.en.vtt` | English captions for the narration and music |
| `website/media/transcript.txt` | Narration and descriptions of the application scenes |
| `website/media/credits.txt` | Narration/music provenance and the supplied soundfont license notice |
| `website/media/screens/*.webp` | Six application stills from the recording, 960 and 1600 pixels wide; each links to its full-size file |
| `.github/workflows/pages.yml` | Publishes only `website/` through GitHub Pages |

To update the recording, obtain the intended final narrated MP4 and poster from
the presentation author and copy them into these website paths. Refresh captions,
the descriptive transcript and the credits from that recording and its source
materials.
Do not publish a silent intermediate as the final presentation. Compare hashes
with the selected output and check playback, sound, captions and seeking.

When the recording changes, re-extract the stills and check that each frame still shows
the described screen without a dialog, highlight or transition. The current stills crop the
1920×1080 recording to its application window at these offsets (seconds): `orders` 18.5,
`bench-run` 43, `result-review` 63, `report-revisions` 84, `delivery` 86 and
`audit-trail` 98. From the repository root, with FFmpeg and `cwebp`:

```bash
ffmpeg -ss 18.5 -i website/media/vibe-tox-overview.mp4 -frames:v 1 \
  -vf "crop=1804:948:58:111" /tmp/orders.png
for width in 960 1600; do
  cwebp -q 82 -m 6 -resize "$width" 0 /tmp/orders.png \
    -o "website/media/screens/orders-$width.webp"
done
```

Update the image `alt` text and captions in `index.html` if a still shows a different screen.

Preview from the repository root:

```bash
python3 -m http.server 8766 --bind 127.0.0.1 --directory website
```

Open `http://127.0.0.1:8766/`. Check desktop and narrow mobile widths, keyboard
navigation, actual playback, captions, transcript and download links. Test without
JavaScript: all page content and controls use browser-native behavior.

Formatting and source checks:

```bash
npx biome check website
git diff --check
```

The video has explicit dimensions, native controls, captions, `preload="none"`, and
progressive `loading="lazy"`. Browsers without media lazy-loading support retain the
ordinary player and preload behavior. Playback begins only when the visitor presses play. The player starts unmuted so
the supplied narration is heard; its native controls manage volume and captions.
If playback fails, use the always-visible transcript or download the MP4.

Review website copy against the canonical overview and application behavior.
Do not advance a built-in help topic's `reviewedOn` merely because website copy
changed; that date requires review of the topic against its implementation.

## Evidence

Primary sources checked 2026-10-14. These are hosting and web-platform facts, not
regulatory conclusions; no jurisdiction-specific rule is introduced.

| ID | Publisher and direct source | Application and limits |
| --- | --- | --- |
| WEB-01 | GitHub, [publishing sources](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site) | Actions can publish the `website/` artifact. Branch sources accept root or `/docs`; the latter would mix the website with canonical project documentation. Publication date not stated. |
| WEB-02 | GitHub, [Pages limits](https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits) | Published site maximum 1 GB; soft bandwidth limit 100 GB/month. Recheck before distributing larger media. Publication date not stated. |
| WEB-03 | GitHub, [large files](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-large-files-on-github) and [Git LFS](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-git-large-file-storage) | Git blocks individual files over 100 MiB; Pages does not support LFS. Keep the short video directly in Git well below that threshold. Publication dates not stated. |
| WEB-04 | GitHub, [static workflow](https://github.com/actions/starter-workflows/blob/main/pages/static.yml) and [Pages API](https://docs.github.com/en/rest/pages/pages#create-a-github-pages-site) | Official artifact/deployment workflow and `build_type=workflow` configuration. Versions checked at implementation time. |
| WEB-05 | Apple, [macOS Tahoe software license](https://www.apple.com/legal/sla/docs/macOSTahoe.pdf), System Voices section | System voice use for original content is limited to personal, noncommercial projects; public sharing is excluded. The supplied video uses Kokoro narration and original music under the notices in its media credits; this Apple license is not permission to substitute system-voice narration in public media. |

The Modern Web Guidance `lazy-load-video-and-audio` guide was retrieved on the same
date. It informed explicit player dimensions, deliberate preload behavior and the
native fallback; media lazy loading is a progressive enhancement.
