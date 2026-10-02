import { createSignal, type JSX } from "solid-js";
import { copyText } from "../lib/format";

export function Field(props: { label: string; children: JSX.Element }) {
  return (
    <label class="field">
      <span class="label-text">{props.label}</span>
      {props.children}
    </label>
  );
}

export function CopyButton(props: { text: () => string; label?: string }) {
  const [copied, setCopied] = createSignal(false);
  let timer: number | undefined;
  return (
    <button
      type="button"
      class="btn copy-btn"
      onClick={async () => {
        if (await copyText(props.text())) {
          setCopied(true);
          clearTimeout(timer);
          timer = window.setTimeout(() => setCopied(false), 1600);
        }
      }}
    >
      {copied() ? "Copied" : (props.label ?? "Copy")}
    </button>
  );
}
