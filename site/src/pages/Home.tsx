import sample from "../data/sample.json";
import { type Sample } from "../lib/wasm";
import { hrpOf } from "../lib/format";
import { A } from "@solidjs/router";
import { onCleanup, onMount } from "solid-js";
import { buzzFault } from "../lib/buzz";

const fixture = sample as Sample;

const rand = (lo: number, hi: number) => lo + Math.random() * (hi - lo);
const pick = <T,>(xs: readonly [T, ...T[]]) =>
	xs[Math.floor(Math.random() * xs.length)] ?? xs[0];

// a failing tube does three things: it stutters in a burst of uneven cuts, it
// drops out hard to near dark and snaps back, and it sags for a second with
// a shaky floor. Every cut is a hard step, nothing eases.
const HOLD = "steps(1, end)";

function stutter(): Keyframe[] {
	const cuts = Math.round(rand(4, 9));
	const frames: Keyframe[] = [{ opacity: 1, offset: 0, easing: HOLD }];
	for (let i = 1; i < cuts; i++) {
		frames.push({
			opacity: i % 2 ? rand(0.04, 0.55) : rand(0.65, 1),
			offset: i / cuts,
			easing: HOLD,
		});
	}
	frames.push({ opacity: 1, offset: 1 });
	return frames;
}

function dropout(): Keyframe[] {
	const dark = rand(0.03, 0.12);
	const hold = rand(0.2, 0.55);
	return [
		{ opacity: 1, offset: 0, easing: HOLD },
		{ opacity: dark, offset: 0.08, easing: HOLD },
		{ opacity: dark, offset: 0.08 + hold, easing: HOLD },
		{ opacity: rand(0.5, 0.85), offset: 0.16 + hold, easing: HOLD },
		{ opacity: dark, offset: 0.2 + hold, easing: HOLD },
		{ opacity: 1, offset: 1 },
	];
}

function sag(): Keyframe[] {
	const floor = rand(0.45, 0.7);
	const steps = 14;
	const frames: Keyframe[] = [{ opacity: 1, offset: 0 }];
	for (let i = 1; i < steps; i++) {
		frames.push({
			opacity: floor + rand(-0.1, 0.1),
			offset: i / steps,
			easing: HOLD,
		});
	}
	frames.push({ opacity: 1, offset: 1 });
	return frames;
}

type Fault = () => [Keyframe[], number];

const FAULTS: [Fault, ...Fault[]] = [
	() => [stutter(), rand(260, 720)],
	() => [stutter(), rand(260, 720)],
	() => [dropout(), rand(320, 900)],
	() => [sag(), rand(900, 1900)],
];

// faults cluster: a third of the time the next one follows within half a second
function malfunction(tube: HTMLElement) {
	let timer: number;
	const schedule = (delay: number) => {
		timer = window.setTimeout(() => {
			const [frames, duration] = pick(FAULTS)();
			tube.animate(frames, { duration });
			buzzFault(duration);
			schedule(Math.random() < 0.33 ? rand(120, 500) : rand(1200, 6000));
		}, delay);
	};
	schedule(rand(2000, 4500));
	return () => clearTimeout(timer);
}

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
	let tube!: HTMLSpanElement;
	onMount(() => {
		if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
		onCleanup(malfunction(tube));
	});
	return (
		<main>
			<section class="hero">
				<h1>
					<span ref={tube}>Sapling</span> note disclosures, decoded in your
					browser.
				</h1>
				<p class="lede">
					A disclosure lets a user share that a transaction paid a given amount
					to a given address.
				</p>
				<div class="btn-row">
					<A
						href="/create"
						class="btn primary"
						style={{ "text-decoration": "none" }}
					>
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
						<span class="desc">
							Check a Disclosure against its transaction.
						</span>
						<span class="arrow">→</span>
					</A>
				</nav>
			</section>
		</main>
	);
}
