# Build Battle website

Static SvelteKit site that shows every battle in `../builds/<slug>/*.json` as interactive 3D
(three.js): orbit the builds, replay the block-by-block build animation, pick a favorite
(remembered in the browser). Kid-friendly, no backend.

The voxelizer (`src/lib/voxels.ts`) is a port of the Rust DSL in `../src/voxels.rs`, so the
site shows exactly what the videos show. Themes mirror `Theme::by_name` in `../src/render.rs`.

## Develop

```sh
mise run web            # or: cd web && npm install && npm run dev
npm run check           # svelte-check / TypeScript
npm run build           # static site in build/
npm run preview         # serve the production build
```

## Adding a battle

Drop `1.json`, `2.json`, `3.json` into `../builds/<slug>/` and rebuild. Title, emoji, theme and
tagline come from `META` in `src/lib/builds.ts` (unknown slugs get a title from the folder name
and the `day` theme).

## Links

- `/`: gallery of all battles
- `/b/<slug>`: one battle; `/b/<slug>#play` starts all build animations on load (good for sharing)

## Hosting

`build/` is plain static files (one HTML file per page), so it can be hosted on GitHub Pages,
Netlify, Cloudflare Pages or any static host. When serving from a sub-path (e.g. GitHub Pages
project sites), set `paths.base` in `vite.config.ts`.
