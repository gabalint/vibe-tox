# Public website and presentation

The presentation is published at
[openbga.github.io/steady-state-lims](https://openbga.github.io/steady-state-lims/) from the public
repository [openbga/steady-state-lims](https://github.com/openbga/steady-state-lims); see
[publication](#publication).

The website summarizes the [license](../architecture/decisions/0017-gplv3-license.md) and links the
[development disclaimer](../governance/development-disclaimer.md). The page states the development
status once in the hero and once in its vibe-coded disclaimer, and labels the recording and its
stills as demo data; the video carries its own notices. Keep these current without repeating them
in other sections. Signed releases, hosted services and clinical readiness still require their
own approvals (Q-11, Q-19 in the [decision queue](../planning/open-questions.md)). The Kokoro and
soundfont notices in [media credits](../../website/media/credits.txt) retain their own terms.

## Publication

GitHub Pages uses **GitHub Actions** as its publishing source. Changes to `website/`
or `.github/workflows/pages.yml` on `main` trigger publication, and
`workflow_dispatch` supports a manual deployment. The workflow uploads the already
prepared static files, so neither the Rust engine nor the desktop UI must build to
publish the presentation. Permissions are limited to reading source and deploying
Pages with an OIDC token. A website change on `develop` goes live when it reaches
`main` ([branches](releases.md#branches)), and is then public within minutes;
review it in the local preview first. This guide does not attest to a current
remote-state check.

## Content and design

The canonical product copy is [the overview](../overview.md), checked against the
[implementation plan](../planning/implementation-plan.md) and
[operator guide](../operations/README.md). The intended readers are laboratory owners,
potential collaborators, and developers evaluating the project. The page moves from
the single-computer proposition to the video, the workflow shown through application
screens, data ownership, common questions, downloading installers or building from source,
development status with the license and disclaimer, and the repository. Its actions open the
latest development release, presentation, documentation, contribution and security
guidance, the support link that [AGENTS.md](../../AGENTS.md#current-mandate)
permits, or source. The hero's “Download development build” action and the
getting-started section link to the [latest release](https://github.com/openbga/steady-state-lims/releases/latest),
where visitors choose their platform's installer and read its release notes. Keep
the destination independent of versioned asset filenames; [branches and releases](releases.md)
describes the unsigned development builds and the separate nightly channel.
The footer credits OpenBGA and carries one small support notice: “Are you looking
for support? Contact bgasoft.com.” The domain links to <https://bgasoft.com>.

### Search and sharing metadata

The page targets what laboratories search for, such as a free toxicology LIS or LIMS. The
`<title>`, meta description, first heading and Open Graph tags use those terms in readable
copy, without keyword lists (WEB-06). The questions section answers cost, LIS versus LIMS,
intended laboratories and how to try the application. The [README](../../README.md) opens
with the same terms because GitHub search and the repository page index it. A JSON-LD
block in `index.html` describes the application (`SoftwareApplication`: free offer,
platforms, GPL-3.0-only) and the presentation (`VideoObject`). Keep its facts in step with
the visible copy. Google shows a software rich result only with ratings or reviews; the
project has none and must not invent them (WEB-07). The video's `uploadDate` is the publication date of the current branding edit,
2026-11-11. Update it when a different recording is published, and keep `duration`
matched to that file.

A project site at `openbga.github.io/steady-state-lims/` is a subdirectory of the host. Google
reads `robots.txt` only from the host root and does not support site names for
subdirectories (WEB-08), so the site ships neither a `robots.txt` nor `WebSite` markup.
Search Console verification and sitemap submission would be project-owner actions outside
the repository. The repository's GitHub description and topics carry the same terms and are
edited in the repository settings.

The website inherits the application's blue/neutral palette and system typography
from `ui/src/styles/tokens.css`, with larger type and spacing for reading. It uses
native tables and a video player, clear focus indicators, and responsive layouts.
The hero's workflow illustration describes real stages; it is not an application
screenshot. The workflow section shows six stills cropped from the edited recording
described below; the note beneath them labels them as demo data from a mocked engine.
Keep it with the stills. The build-from-source
commands follow the [developer guide](README.md#build-and-run); update both together.
No analytics, contact form, account, clinical interface, or JavaScript runtime is used.

The site serves `steady-state-lims-overview.mp4` and its poster. It is a 121.233-second
recording of the actual React interface with a mocked engine and synthetic data,
with narration and music. It demonstrates a workflow, not end-to-end clinical
execution or validation.

The title cards, app header, window title and two spoken product-name phrases
use Steady State LIMS. The workflow footage, synthetic data and remaining narration
come from the supplied demonstration. In the recorded header, the search field
sits further right to make room for the name, as the application lays it out. The
replacement phrases use Kokoro v1.0 with the same `af_heart` voice; LIMS is
pronounced as one word, “limz.” The
[media credits](../../website/media/credits.txt) record the editing and audio
provenance. The poster and stills come from this edited recording.

English captions, a descriptive transcript and audio credits accompany the video.
Keep development status and platform verification limits accurate when changing
the page or choosing a later recording.

## Files and updates

| File | Purpose |
| --- | --- |
| `website/index.html` | Public content, search metadata and structured data, navigation, native player and links |
| `website/styles.css`, `website/responsive.css` | Presentation styles and responsive adaptations |
| `website/favicon.png` | Application test-tube icon, reused from `src-tauri/icons/128x128.png` |
| `website/media/steady-state-lims-overview.mp4` | Narrated H.264/AAC application walkthrough with current branding; progressive playback |
| `website/media/steady-state-lims-overview-poster.jpg` | Title poster from the edited recording |
| `website/media/steady-state-lims-overview.en.vtt` | English captions for the narration and music |
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
ffmpeg -ss 18.5 -i website/media/steady-state-lims-overview.mp4 -frames:v 1 \
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

Search sources were checked 2026-11-05; they describe Google Search behavior only.

| ID | Publisher and direct source | Application and limits |
| --- | --- | --- |
| WEB-06 | Google Search Central, [title links](https://developers.google.com/search/docs/appearance/title-link) and [snippets and meta descriptions](https://developers.google.com/search/docs/appearance/snippet) | Use a concise, descriptive title, a prominent first heading and an accurate page summary; repeated terms and keyword lists are discouraged. No length limit; results truncate to the device width. Publication date not stated. |
| WEB-07 | Google Search Central, [software app](https://developers.google.com/search/docs/appearance/structured-data/software-app) and [video](https://developers.google.com/search/docs/appearance/structured-data/video) structured data | Software rich results require `name`, `offers.price` and `aggregateRating` or `review`. Video requires `name`, `thumbnailUrl` and `uploadDate`; `contentUrl`, `description` and ISO 8601 `duration` are recommended. Eligibility does not guarantee display. Publication dates not stated. |
| WEB-08 | Google Search Central, [create a robots.txt file](https://developers.google.com/search/docs/crawling-indexing/robots/create-robots-txt) and [site names](https://developers.google.com/search/docs/appearance/site-names) | `robots.txt` must be at the root of the host and cannot be placed in a subdirectory. Site names are not supported at the subdirectory level. Publication dates not stated. |

The Modern Web Guidance `lazy-load-video-and-audio` guide was retrieved on
2026-10-14. It informed explicit player dimensions, deliberate preload behavior and the
native fallback; media lazy loading is a progressive enhancement.
