import { createEffect, onCleanup, onMount, type JSX } from "solid-js";
import { type RouteSectionProps, A, Route, Router, useLocation } from "@solidjs/router";
import { render } from "solid-js/web";
import "./styles.css";
import Home from "./pages/Home";
import Inspect from "./pages/Inspect";
import Verify from "./pages/Verify";
import Create from "./pages/Create";
import { metaFor } from "./meta";

const between = (lo: number, hi: number) => lo + Math.random() * (hi - lo);

// the pitch of the board's pixels, the same as the mask in styles.css
const CELL = 7;
const SVG = "http://www.w3.org/2000/svg";

// a random walk across the grid: a column to the right most steps, a row up
// or down now and then, never back
function walk(cols: number, rows: number) {
  const cells: Array<[number, number]> = [];
  let row = Math.floor(between(1, rows - 1));
  for (let col = 0; col < cols; ) {
    cells.push([col, row]);
    const roll = Math.random();
    if (roll < 0.15 && row > 0) row--;
    else if (roll < 0.3 && row < rows - 1) row++;
    else col++;
  }
  return cells;
}

// each cell on the path lights in turn and fades, so a short tail follows
// the head across the board
function cross(trail: SVGSVGElement) {
  const { width, height } = trail.getBoundingClientRect();
  const dots = walk(Math.ceil(width / CELL), Math.floor(height / CELL)).map(([col, row], i) => {
    const dot = document.createElementNS(SVG, "circle");
    dot.setAttribute("cx", `${(col + 0.5) * CELL}`);
    dot.setAttribute("cy", `${(row + 0.5) * CELL}`);
    dot.setAttribute("r", `${CELL / 2}`);
    trail.append(dot);
    return dot.animate([{ opacity: 1 }, { opacity: 1, offset: 0.3 }, { opacity: 0 }], {
      duration: 600,
      delay: i * 12,
    });
  });
  Promise.all(dots.map((a) => a.finished)).then(() => trail.replaceChildren());
}

function passes(trail: SVGSVGElement) {
  let timer: number;
  const schedule = () => {
    timer = window.setTimeout(() => {
      cross(trail);
      schedule();
    }, between(4000, 12000));
  };
  schedule();
  return () => clearTimeout(timer);
}

function Status() {
  let trail!: SVGSVGElement;
  onMount(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    onCleanup(passes(trail));
  });
  return (
    <aside class="status" aria-label="Status">
      <div class="status-inner">
        <span class="status-board" aria-hidden="true">
          <span class="status-lamp" />
          <svg class="status-trail" ref={trail} />
        </span>
        <span class="status-badge">Draft</span>
        <p>
          Sapling is complete as a draft, not final.{" "}
          <a href="https://zips.z.cash/zip-0304">ZIP 304</a> and{" "}
          <a href="https://zips.z.cash/zip-0311">ZIP 311</a> are drafts. No test vectors
          exist.
        </p>
      </div>
    </aside>
  );
}

function App(props: RouteSectionProps) {
  const location = useLocation();
  let nav: HTMLElement | undefined;
  let thumb: HTMLElement | undefined;

  const place = () => {
    const active = nav?.querySelector<HTMLAnchorElement>("a.active");
    if (!active || !thumb) return;
    thumb.style.width = `${active.offsetWidth}px`;
    thumb.style.transform = `translateX(${active.offsetLeft}px)`;
  };

  onMount(() => {
    place();
    document.fonts.ready.then(place);
    window.addEventListener("resize", place);
    onCleanup(() => window.removeEventListener("resize", place));
  });

  createEffect(() => {
    document.title = metaFor(location.pathname).title;
    place();
  });

  return (
    <div class="shell">
      <header class="top">
        <div class="top-inner">
          <A href="/" class="wordmark" draggable={false}>
            <img src="/icon.svg" width="22" height="22" alt="" />
            <span class="wordmark-text">zcash-disclosure</span>
          </A>
          <nav class="segmented" ref={(el) => (nav = el)}>
            <span class="segmented-thumb" ref={(el) => (thumb = el)} aria-hidden="true" />
            <A href="/" end activeClass="active" draggable={false}>
              Overview
            </A>
            <A href="/create" activeClass="active" draggable={false}>
              Create
            </A>
            <A href="/inspect" activeClass="active" draggable={false}>
              Inspect
            </A>
            <A href="/verify" activeClass="active" draggable={false}>
              Verify
            </A>
          </nav>
        </div>
      </header>
      <Status />
      {props.children}
    </div>
  );
}

render(
  () => (
    <Router root={App}>
      <Route path="/" component={Home} />
      <Route path="/inspect" component={Inspect} />
      <Route path="/verify" component={Verify} />
      <Route path="/create" component={Create} />
    </Router>
  ),
  document.getElementById("root")!,
);
