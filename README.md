# DriveSync website

Public website for DriveSync, an independent Google Drive client for Linux.
This standalone `web` branch contains the website; application development remains on `main`.

## Development

Requires Node.js 22.12+ (or a supported newer LTS) and npm.

```sh
npm ci
npm run dev
npm run lint
npm run build
npm run preview
```

Open the printed local URL under `/drivesync/`. Vite builds the application,
development, donations and privacy pages as separate HTML entry points. Each URL
can be opened directly on GitHub Pages.

## Components and appearance

React, TypeScript, Vite and Tailwind CSS. The shadcn/ui components use Base UI
and the `base-luma` style, following RSCFlow's component approach. Theme tokens in
`src/index.css` define neutral surfaces and GNOME-inspired blue accents. Cantarell
fonts (Latin regular and bold, via Fontsource) and screenshots are served locally; no external font service is used.

## Publication

The GitHub Actions workflow validates and builds pushes to `web`, then deploys
`dist/` to GitHub Pages. Repository Settings → Pages must use GitHub Actions as
the publishing source. The expected project URL is
https://rafatosta.github.io/drivesync/.

## Content

Application claims should track the current `main` README and verified behavior.
The application page includes a gallery of 28 screens grouped into first steps, accounts and navigation, transfers, and status messages. Visitors can select thumbnails, step through each group, switch between wide and compact captures when available, and enlarge the selected image. Screenshots use fictional accounts and simulated states from the application UI.
Donation channels are not configured; do not add payment details without the
maintainer's direction. Update the privacy policy when data practices change.
No analytics, contact form, tracking cookies or payment processing are included.
Theme preference alone is saved in the browser's local storage.
