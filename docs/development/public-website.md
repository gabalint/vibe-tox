# Public website

The website is published at
[openbga.github.io/tox](https://openbga.github.io/tox/) from the public
repository [openbga/tox](https://github.com/openbga/tox); see
[publication](#publication).

The website summarizes the [license](../architecture/decisions/0021-agplv3-license.md), the
copyright and trademark notice in [NOTICE](../../NOTICE), and links the
[development disclaimer](../governance/development-disclaimer.md). The page states the development
status once in the hero and once in its vibe-coded disclaimer, and labels the application
screens as demo data. Keep these current without repeating them in other sections. Signed
releases, hosted services and clinical readiness still require their own approvals (Q-11,
Q-19 in the [decision queue](../planning/open-questions.md)).

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
the single-computer proposition to the workflow shown through application screens, data ownership, common questions, downloading installers or building from source,
development status with the license and disclaimer, and the repository. Its actions open the
latest development release, the workflow section, documentation, contribution and security
guidance, the commercial support link that [AGENTS.md](../../AGENTS.md#current-mandate)
permits, or source. The hero's “Download development build” action and the
getting-started section link to the [latest release](https://github.com/openbga/tox/releases/latest),
where visitors choose their platform's installer and read its release notes. Keep
the destination independent of versioned asset filenames; [branches and releases](releases.md)
describes the unsigned development builds and the separate nightly channel.

### Search and sharing metadata

The page targets what laboratories search for, such as a free toxicology LIS or LIMS. The
`<title>`, meta description, first heading and Open Graph tags use those terms in readable
copy, without keyword lists (WEB-06). The questions section answers cost, LIS versus LIMS,
intended laboratories and how to try the application. The [README](../../README.md) opens
with the same terms because GitHub search and the repository page index it. A JSON-LD
block in `index.html` describes the application (`SoftwareApplication`: free offer,
platforms, AGPL-3.0-only, BGASoft Inc. as publisher and copyright holder). Keep its facts in
step with the visible copy. Google shows a software rich result only with ratings or reviews;
the project has none and must not invent them (WEB-07). The Open Graph image is
`media/openbga-tox-social.jpg`, rendered from `media/openbga-tox-social.svg`.

A project site at `openbga.github.io/tox/` is a subdirectory of the host. Google
reads `robots.txt` only from the host root and does not support site names for
subdirectories (WEB-08), so the site ships neither a `robots.txt` nor `WebSite` markup.
Search Console verification and sitemap submission would be project-owner actions outside
the repository. The repository's GitHub description and topics carry the same terms and are
edited in the repository settings.

The website inherits the application's blue/neutral palette and system typography
from `ui/src/styles/tokens.css`, with larger type and spacing for reading. It uses
native tables, clear focus indicators, and responsive layouts.
The hero's workflow illustration describes real stages; it is not an application
screenshot. The workflow section shows six application screens captured from the React
interface with a mocked engine and synthetic data; the note beneath them labels them as demo
data from a mocked engine. Keep it with the screens. They demonstrate a workflow, not
end-to-end clinical execution or validation. The build-from-source
commands follow the [developer guide](README.md#build-and-run); update both together.
No analytics, contact form, account, clinical interface, or JavaScript runtime is used.

## Files and updates

| File | Purpose |
| --- | --- |
| `website/index.html` | Public content, search metadata and structured data, navigation and links |
| `website/styles.css`, `website/responsive.css` | Presentation styles and responsive adaptations |
| `website/favicon.svg` | Test-tube symbol of the application icon (`src-tauri/app-icon.svg`) on the website blue |
| `website/media/openbga-tox-social.svg`, `.jpg` | Open Graph image source and its 1920×1080 rendering |
| `website/media/screens/*.webp` | Six application screens, 960 and 1600 pixels wide; each links to its full-size file |
| `.github/workflows/pages.yml` | Publishes only `website/` through GitHub Pages |

The header wordmark is the same test-tube symbol, inline in `index.html`, beside the
product name. After changing the social image source, render it at 1920×1080, for example
with headless Chrome, and save it as a JPEG:

```bash
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new \
  --hide-scrollbars --window-size=1920,1080 --screenshot=/tmp/social.png \
  "file://$PWD/website/media/openbga-tox-social.svg"
ffmpeg -i /tmp/social.png -q:v 3 website/media/openbga-tox-social.jpg
```

The application screens are browser captures of the React interface (`npm run dev` style
Vite server) with a mock of the Tauri engine bridge answering with synthetic fixtures: a
fictional laboratory, staff, clinics and patients, and a fixed clock. The mock harness is
local tooling, not part of the repository. Capture each screen at 1920×1009 CSS pixels in
the light appearance, with no dialog, toast or hover state, then scale it:

```bash
for width in 960 1600; do
  cwebp -q 82 -m 6 -resize "$width" 0 /tmp/orders.png \
    -o "website/media/screens/orders-$width.webp"
done
```

Check that each capture shows the described screen with the product name in the header.

Update the image `alt` text and captions in `index.html` if a screen shows a different view.

Preview from the repository root:

```bash
python3 -m http.server 8766 --bind 127.0.0.1 --directory website
```

Open `http://127.0.0.1:8766/`. Check desktop and narrow mobile widths, keyboard
navigation, screen links and download links. Test without
JavaScript: all page content and controls use browser-native behavior.

Formatting and source checks:

```bash
npx biome check website
git diff --check
```

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

Search sources were checked 2026-11-05; they describe Google Search behavior only.

| ID | Publisher and direct source | Application and limits |
| --- | --- | --- |
| WEB-06 | Google Search Central, [title links](https://developers.google.com/search/docs/appearance/title-link) and [snippets and meta descriptions](https://developers.google.com/search/docs/appearance/snippet) | Use a concise, descriptive title, a prominent first heading and an accurate page summary; repeated terms and keyword lists are discouraged. No length limit; results truncate to the device width. Publication date not stated. |
| WEB-07 | Google Search Central, [software app](https://developers.google.com/search/docs/appearance/structured-data/software-app) structured data | Software rich results require `name`, `offers.price` and `aggregateRating` or `review`. Eligibility does not guarantee display. Publication dates not stated. |
| WEB-08 | Google Search Central, [create a robots.txt file](https://developers.google.com/search/docs/crawling-indexing/robots/create-robots-txt) and [site names](https://developers.google.com/search/docs/appearance/site-names) | `robots.txt` must be at the root of the host and cannot be placed in a subdirectory. Site names are not supported at the subdirectory level. Publication dates not stated. |
