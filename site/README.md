# Plugget website

The landing page for plugget.dev. Source lives alongside the Rust CLI in this repository.

```sh
npm ci
npm run dev
```

Validation and production build:

```sh
npm run lint
npx tsc --noEmit
npm run build
```

The page uses React, Vinext and the Sites Cloudflare integration. Edit `app/page.tsx` for content, `app/globals.css` for the theme, and `app/layout.tsx` for metadata. The copy-install button uses the browser Clipboard API and displays a manual-copy fallback when unavailable.

The generated UI catalog is retained in `components/ui`; lint targets authored application code, while TypeScript checks the complete project. Dependency versions were updated to resolve the starter's security advisories; npm audit reported zero vulnerabilities at implementation time.

Publishing uses the project registered in `.openai/hosting.json`. No credentials belong in this repository. The custom domain must be connected through its DNS provider before plugget.dev can serve the site.
