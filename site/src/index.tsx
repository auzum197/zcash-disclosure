import { createEffect, onCleanup, onMount, type JSX } from "solid-js";
import { type RouteSectionProps, A, Route, Router, useLocation } from "@solidjs/router";
import { render } from "solid-js/web";
import "./styles.css";
import Home from "./pages/Home";
import Inspect from "./pages/Inspect";
import Verify from "./pages/Verify";
import Create from "./pages/Create";
import { metaFor } from "./meta";

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
