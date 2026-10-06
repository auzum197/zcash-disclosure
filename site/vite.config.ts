import { defineConfig, type Plugin } from "vite";
import solid from "vite-plugin-solid";
import { ORIGIN, ROUTES, SITE, metaFor, urlOf, type Meta } from "./src/meta";

const escape = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

function head(m: Meta) {
  const image = `${ORIGIN}/og/${m.slug}.png`;
  const tags = [
    `<title>${escape(m.title)}</title>`,
    `<meta name="description" content="${escape(m.description)}" />`,
    `<link rel="canonical" href="${urlOf(m)}" />`,
    `<meta property="og:type" content="website" />`,
    `<meta property="og:site_name" content="${SITE}" />`,
    `<meta property="og:locale" content="en_US" />`,
    `<meta property="og:url" content="${urlOf(m)}" />`,
    `<meta property="og:title" content="${escape(m.headline)}" />`,
    `<meta property="og:description" content="${escape(m.description)}" />`,
    `<meta property="og:image" content="${image}" />`,
    `<meta property="og:image:type" content="image/png" />`,
    `<meta property="og:image:width" content="1200" />`,
    `<meta property="og:image:height" content="630" />`,
    `<meta property="og:image:alt" content="${escape(m.alt)}" />`,
    `<meta name="twitter:card" content="summary_large_image" />`,
    `<meta name="twitter:title" content="${escape(m.headline)}" />`,
    `<meta name="twitter:description" content="${escape(m.description)}" />`,
    `<meta name="twitter:image" content="${image}" />`,
    `<meta name="twitter:image:alt" content="${escape(m.alt)}" />`,
  ];
  return tags.map((t) => `    ${t}`).join("\n");
}

const MARK = "<!-- meta -->";

// a crawler does not run the app, so every route gets its own HTML with its
// own head: in dev from the request path, in a build as create.html,
// inspect.html and verify.html next to index.html. GitHub Pages serves
// /create from create.html.
function meta(): Plugin {
  return {
    name: "route-meta",
    transformIndexHtml: {
      order: "pre",
      handler: (html, ctx) => html.replace(MARK, `${MARK}\n${head(metaFor(ctx.originalUrl?.split("?")[0] ?? "/"))}\n    <!-- /meta -->`),
    },
    generateBundle: {
      order: "post",
      handler(_, bundle) {
        const index = bundle["index.html"];
        if (index?.type !== "asset") return;
        const html = String(index.source);
        const block = /<!-- meta -->[\s\S]*?<!-- \/meta -->/;
        for (const m of ROUTES.slice(1)) {
          this.emitFile({
            type: "asset",
            fileName: `${m.slug}.html`,
            source: html.replace(block, `${MARK}\n${head(m)}\n    <!-- /meta -->`),
          });
        }
      },
    },
  };
}

export default defineConfig({
  plugins: [solid(), meta()],
  build: {
    target: "es2022",
  },
});
