// what each route says about itself to a browser tab and to a link preview.
// vite.config.ts writes these into the head of every route's HTML, the app
// sets the title on navigation, and scripts/og.mjs draws the images.
export const ORIGIN = "https://zip311.auzum197.com";
export const SITE = "zcash-disclosure";

export type Meta = {
  path: string;
  slug: string;
  title: string;
  headline: string;
  line: string;
  description: string;
  alt: string;
};

export const ROUTES: [Meta, ...Meta[]] = [
  {
    path: "/",
    slug: "home",
    title: `${SITE} · Sapling note disclosures`,
    headline: "Sapling note disclosures, decoded in your browser",
    line: "One string opens chosen outputs of one transaction.",
    description:
      "Decode, verify and create Zcash Sapling note disclosures in your browser. One string reveals chosen outputs of one transaction.",
    alt: "The zcash-disclosure sign, Sapling note disclosures decoded in your browser, over a line globe.",
  },
  {
    path: "/create",
    slug: "create",
    title: `Create a disclosure · ${SITE}`,
    headline: "Create a disclosure",
    line: "Build a disclosure from a viewing key.",
    description:
      "Build a disclosure for chosen Sapling outputs of a transaction from a viewing key.",
    alt: "Create, in dot-matrix type, over a line globe.",
  },
  {
    path: "/inspect",
    slug: "inspect",
    title: `Inspect a disclosure · ${SITE}`,
    headline: "Inspect a disclosure",
    line: "Decode a disclosure offline.",
    description:
      "Decode a disclosure string offline and see its network, its transaction ID and the Sapling outputs it opens.",
    alt: "Inspect, in dot-matrix type, over a line globe.",
  },
  {
    path: "/verify",
    slug: "verify",
    title: `Verify a disclosure · ${SITE}`,
    headline: "Verify a disclosure",
    line: "Check a disclosure against its transaction.",
    description:
      "Check a disclosure against its transaction and read the value, address and memo of each output it opens, in your browser.",
    alt: "Verify, in dot-matrix type, over a line globe.",
  },
];

export const metaFor = (path: string) =>
  ROUTES.find((r) => r.path === (path.replace(/\/+$/, "") || "/")) ?? ROUTES[0];

export const urlOf = (m: Meta) => `${ORIGIN}${m.path === "/" ? "/" : m.path}`;
