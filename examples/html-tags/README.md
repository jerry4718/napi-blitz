# html-tags

Manual visual smoke test for HTML tag handling in `@ylcc/napi-blitz`.

The matrix is organized from Blitz's upstream UA stylesheet:

- `.ignore/blitz/packages/blitz-dom/assets/default.css`
- special layout branches in `.ignore/blitz/packages/blitz-dom/src/layout/construct.rs`

Run:

```sh
cd examples/html-tags
pnpm start
```

This intentionally uses the standard DOM API directly instead of Vue, so failures
point at host/native DOM integration rather than a renderer abstraction.
