# torc-dash web UI

Svelte 5 + TypeScript source for the torc-dash dashboard. `npm run build` writes the bundle to
`../static/app/`, which `torc-dash` embeds at compile time and serves at `/`. The built bundle is
checked in so building torc does not require Node.js; rebuild and commit it with any UI change.

## Development

```bash
cd torc-dash/web
npm install
# Run a torc-dash to proxy API calls to (default http://127.0.0.1:8090):
torc-dash --standalone --port 8090 &
npm run dev            # http://localhost:5173/static/app/ with hot reload
npm run check          # type-check (svelte-check)
npm run build          # write ../static/app/, then rebuild torc-dash
npm run gen:api        # regenerate src/lib/api/schema.d.ts after api/openapi.yaml changes
npm test               # Node checks (highlighting, spec editor schema; needs cargo)
```

Set `TORC_DASH_URL` to proxy to a torc-dash on another address.

Design and conventions: `docs/src/specialized/design/dashboard-frontend.md` (UI) and
`docs/src/specialized/design/dashboard.md` (torc-dash server).

## Layout

- `src/App.svelte`: shell (sidebar, top bar, global shortcuts, auto-refresh)
- `src/lib/`: shared state (`state.svelte.ts`), API clients (`api/`), formatting, UI components
  (`ui/`: `DataTable`, `Modal`, `Badge`, toasts, confirm dialog)
- `src/features/<tab>/`: one directory per tab
