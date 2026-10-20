# Switchify PC docs site

The user documentation for Switchify PC, served at https://pc.theswitchify.app. It is an [Astro Starlight](https://starlight.astro.build) site and is independent of the app build.

```bash
cd site
npm ci
npm run dev     # http://localhost:4321
npm run build   # static output in dist/
```

Pages live in `src/content/docs/` as Markdown. The sidebar is defined in `astro.config.mjs`.

## Screenshots

App screenshots live in `src/assets/screenshots/` and are referenced from the pages with relative Markdown image links, so Astro optimises them at build time. Give every image alt text that says what the screenshot shows.

`npm run screenshots` recaptures them from the real UI. It drives Google Chrome headless (set `CHROME_PATH` if Chrome is somewhere else) against a fake Tauri bridge with made-up switches and devices, so it never touches your own switches, pairing or input. Start the app's Vite server first from the repository root:

```bash
npm ci
npx vite --port 5181 --strictPort --host 127.0.0.1
# in another terminal
cd site && npm run screenshots   # APP_URL overrides http://127.0.0.1:5181/
```

Recapture when a change alters a screen that a screenshot shows, and check each image before committing. The Mac setup screenshot comes from the same browser with the platform set to macOS, so it uses the fonts of the computer you capture on.

Any change that users can see in the app must update these pages in the same pull request. Write for switch users and AT professionals: plain language, no claims the app does not back up, and no accuracy claims for word prediction.

Hosting: a Vercel project for the `switchify-pc` repository with its root directory set to `site`, and `pc.theswitchify.app` pointed at it.
