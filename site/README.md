# Switchify PC docs site

The user documentation for Switchify PC, served at https://pc.theswitchify.app. It is an [Astro Starlight](https://starlight.astro.build) site and is independent of the app build.

```bash
cd site
npm ci
npm run dev     # http://localhost:4321
npm run build   # static output in dist/
```

Pages live in `src/content/docs/` as Markdown. The sidebar is defined in `astro.config.mjs`.

Any change that users can see in the app must update these pages in the same pull request. Write for switch users and AT professionals: plain language, no claims the app does not back up, and no accuracy claims for word prediction.

Hosting: a Vercel project for the `switchify-pc` repository with its root directory set to `site`, and `pc.theswitchify.app` pointed at it.
