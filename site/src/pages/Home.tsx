import sample from "../data/sample.json";
import { type Sample } from "../lib/wasm";
import { hrpOf } from "../lib/format";
import { A } from "@solidjs/router";

const fixture = sample as Sample;
const PREFIX = `${hrpOf(fixture.network)}1`;

const GRAMMAR = [
  'disclosure   = hrp "1" payload         ; one string, Bech32m over the payload',
  "payload      = version txid 1*64item   ; items strictly increase by (typecode, index)",
  "version      = %x01",
  "txid         = 32OCTET                ; internal byte order",
  "item         = typecode length body",
  "typecode     = compactsize",
  "length       = compactsize",
  "body         = index pk_d secret",
  "index        = compactsize             ; position in the Sapling outputs",
  "pk_d         = 32OCTET",
  "secret       = 32OCTET                ; rseed, esk or K_enc",
  'hrp          = "zdu" / "zdutest" / "zduregtest"',
  "compactsize  = 1*9OCTET               ; Bitcoin variable-length integer",
];

const CAPABILITY_COLUMNS = [
  "Capability",
  "Key necessary",
  "Sapling",
  "Orchard / Ironwood",
];

const CAPABILITIES = [
  ["Prove what was paid, and to which address", "Viewing key", "Yes", "Yes"],
  ["Prove who paid", "Spending key", "Yes", "No"],
  ["Prove who controls an address", "Spending key", "Yes", "No"],
  ["Bind a message or challenge", "Spending key", "Yes", "No"],
];

function Grammar() {
  return (
    <pre class="codeblock grammar">
      {GRAMMAR.map((line, i) => {
        const c = line.indexOf(";");
        return (
          <>
            {i > 0 ? "\n" : ""}
            {c >= 0 ? (
              <>
                {line.slice(0, c)}
                <span class="gc">{line.slice(c)}</span>
              </>
            ) : (
              line
            )}
          </>
        );
      })}
    </pre>
  );
}

export default function Home() {
  return (
    <main>
      <section class="hero">
        <h1>Sapling note disclosures, decoded in your browser.</h1>
        <p class="lede">
          A disclosure is a way for a ZEC holder to prove that a transaction
          paid a given amount to a given address.
        </p>
        <div class="btn-row">
          <A href="/create" class="btn primary" style={{ "text-decoration": "none" }}>
            Create
          </A>
          <A href="/inspect" class="btn" style={{ "text-decoration": "none" }}>
            Inspect
          </A>
        </div>
      </section>

      <section class="block">
        <h2 class="section-title">Current capabilities</h2>
        <div class="matrix-scroll">
          <table class="matrix">
            <thead>
              <tr>
                {CAPABILITY_COLUMNS.map((column) => (
                  <th scope="col">{column}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {CAPABILITIES.map(([capability, key, ...pools]) => (
                <tr>
                  <th scope="row">{capability}</th>
                  <td>{key}</td>
                  {pools.map((covered) => (
                    <td class={covered === "Yes" ? "yes" : "no"}>{covered}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section class="block">
        <h2 class="section-title">ABNF</h2>
        <div class="stack">
          <Grammar />
          <p class="string-view codeblock">
            <span class="hrp-part">{PREFIX}</span>
            <span class="rest">{fixture.disclosure.slice(PREFIX.length)}</span>
          </p>
        </div>
      </section>

      <section class="block">
        <h2 class="section-title">Tools</h2>
        <nav class="tool-list">
          <A href="/create">
            <span class="name">Create</span>
            <span class="desc">Build a Disclosure from a viewing key.</span>
            <span class="arrow">→</span>
          </A>
          <A href="/inspect">
            <span class="name">Inspect</span>
            <span class="desc">Decode a Disclosure offline.</span>
            <span class="arrow">→</span>
          </A>
          <A href="/verify">
            <span class="name">Verify</span>
            <span class="desc">Check a Disclosure against its transaction.</span>
            <span class="arrow">→</span>
          </A>
        </nav>
      </section>
    </main>
  );
}
