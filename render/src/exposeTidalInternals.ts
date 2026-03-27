import quartz, { type QuartzPlugin } from "@uwu/quartz";

// Ensure patchAction is loaded on window!
import "./exposeTidalInternals.patchAction";

import { resolveAbsolutePath } from "./helpers/resolvePath";

import { findCreateActionFunction } from "./helpers/findCreateAction";
import { getOrCreateLoadingContainer } from "./loadingContainer";

import { get as idbGet, set as idbSet, createStore } from "idb-keyval";

export const tidalModules: Record<string, object> = {};

const quartzStart = performance.now();
const moduleTimings: Record<string, number> = {};

// Store pending promises to avoid race conditions with circular imports
const pendingModules: Record<string, Promise<object>> = {};

const quartzCache = createStore("@luna/quartzCache", "_");

const fetchCode = async (path: string) => {
	const res = await fetch(path);
	return `${await res.text()}\n//# sourceURL=${path}`;
};

const fetchCodeCached = async (path: string): Promise<string> => {
	const headRes = await fetch(path, { method: "HEAD" });
	const etag = headRes.headers.get("etag") ?? headRes.headers.get("last-modified");

	const cached = await idbGet<{ etag: string; code: string }>(path, quartzCache);
	if (cached?.etag === etag && etag !== null) return cached.code;

	const fullRes = await fetch(path);
	const code = `${await fullRes.text()}\n//# sourceURL=${path}`;

	await idbSet(path, { etag, code }, quartzCache);
	return code;
};

let loading = 0;
const messageContainer = getOrCreateLoadingContainer().messageContainer;

const dynamicResolve: QuartzPlugin["dynamicResolve"] = async ({ name, moduleId, config }) => {
	const path = resolveAbsolutePath(moduleId, name);

	// Skip non-JS files entirely
	if (!path.endsWith(".js") && !path.endsWith(".mjs") && !path.endsWith(".ts")) {
		return {};
	}

	// Skip asset chunks that aren't real modules (images, fonts, css etc embedded in js)
	if (/\/assets\/[^/]+-[a-zA-Z0-9]{8,}\.(css|png|jpg|jpeg|svg|woff2?|ttf|eot)/.test(path)) {
		return {};
	}

	// Return cached module if available
	if (tidalModules[path]) return tidalModules[path];

	// If already loading, wait for the same promise instead of reloading
	if (path in pendingModules) return pendingModules[path];

	// Only log non-asset files to keep the loading screen clean
	if (!path.includes("/assets/")) {
		messageContainer.innerText += `Loading ${path}\n`;
		messageContainer.scrollTop = messageContainer.scrollHeight;
	}
	loading++;

	// Create and store the promise BEFORE starting the async work
	const loadPromise = (async () => {
		const code = await fetchCodeCached(path);

		// Skip files that are purely asset registrations with no real exports
		if (code.length < 500 && !code.includes("export") && !code.includes("module.exports")) {
			tidalModules[path] = {};
			return {};
		}

		const modStart = performance.now();
		const module = await quartz(code, config, path);
		moduleTimings[path] = performance.now() - modStart;

		tidalModules[path] = module;
		return module;
	})();
	pendingModules[path] = loadPromise;

	const result = await loadPromise;
	loading--;

	delete pendingModules[path];
	return result;
};

// Async wait for quartz scripts to be in DOM (needed for tidal-hifi where preload runs before HTML loads)
const waitForScripts = (): Promise<NodeListOf<HTMLScriptElement>> => {
	return new Promise((resolve) => {
		const checkScripts = () => {
			const scripts = document.querySelectorAll<HTMLScriptElement>(`script[type="luna/quartz"]`);
			return scripts.length >= 1 ? scripts : null;
		};
		const setupObserver = () => {
			const observer = new MutationObserver(() => {
				const scripts = checkScripts();
				if (scripts) {
					observer.disconnect();
					resolve(scripts);
				}
			});
			observer.observe(document.documentElement, { childList: true, subtree: true });
		};
		if (document.readyState === "loading") {
			document.addEventListener("DOMContentLoaded", () => {
				const scripts = checkScripts();
				scripts ? resolve(scripts) : setupObserver();
			});
		} else {
			const scripts = checkScripts();
			scripts ? resolve(scripts) : setupObserver();
		}
	});
};

messageContainer.innerText = "Waiting for tidal scripts to load...\n";
const scripts = await waitForScripts();

// Theres usually only 1 script on page that needs injecting (https://desktop.tidal.com/) see native/injector
// So dw about blocking for loop
for (const script of scripts) {
	const scriptPath = new URL(script.src).pathname;

	const scriptContent = await fetchCodeCached(scriptPath);

	// Create and store the promise BEFORE executing quartz to prevent race conditions
	// This ensures that if dynamicResolve is called for this module during execution,
	// it will wait for this same promise instead of loading the module again
	const loadPromise = (async () => {
		const modStart = performance.now();
		const module = await quartz(
			scriptContent,
			{
				// Quartz runs transform > dynamicResolve > resolve
				plugins: [
					{
						transform({ code }) {
							const actionData = findCreateActionFunction(code);

							if (actionData) {
								const { fnName, startIdx } = actionData;
								const funcPrefix = "__LunaUnpatched_";
								const renamedFn = funcPrefix + fnName;

								// Rename the original function declaration by adding a prefix
								// Example: `prepareAction` becomes `__LunaUnpatched_prepareAction`
								code = code.slice(0, startIdx) + funcPrefix + code.slice(startIdx);

								// Assuming the declaration starts 9 characters before the function name
								// (e.g., accounting for "const " or "function ")
								const declarationStartIdx = startIdx - 9;
								const patchedDeclaration = `const ${fnName} = patchAction({ _: ${renamedFn} })._;`;

								// Insert the new patched declaration before the original (now renamed) one
								code = code.slice(0, declarationStartIdx) + patchedDeclaration + code.slice(declarationStartIdx);
							}

							return code;
						},
						dynamicResolve,
						async resolve({ name, moduleId, config, accessor, store }) {
							(store as any).exports = await dynamicResolve({ name, moduleId, config });
							return `${accessor}.exports`;
						},
					},
				],
			},
			scriptPath,
		);
		moduleTimings[scriptPath] = performance.now() - modStart;
		tidalModules[scriptPath] = module;
		return module;
	})();

	// Store the promise BEFORE awaiting it
	pendingModules[scriptPath] = loadPromise;

	// Fetch, transform execute and store the module in moduleCache
	// Hijack the Redux store & inject interceptors
	await loadPromise;

	delete pendingModules[scriptPath];
}

// Print slowest modules
const slowModules = Object.entries(moduleTimings)
	.sort(([, a], [, b]) => b - a)
	.slice(0, 10);

console.group("%c[Luna] Slowest modules to execute", "color: #31d8ff; font-weight: bold;");
for (const [path, ms] of slowModules) {
	const color = ms > 1000 ? "color: red;" : ms > 500 ? "color: orange;" : "color: green;";
	console.log(`%c${ms.toFixed(0)}ms — ${path}`, color);
}
console.groupEnd();

// Hide only after ALL scripts are done
const loadingEl = document.getElementById("tidaluna-loading");
if (loadingEl) {
	messageContainer.innerText += "Done!\n";
	messageContainer.scrollTop = messageContainer.scrollHeight;
	loadingEl.style.transition = "opacity 0.5s ease-out";
	loadingEl.style.opacity = "0";
	setTimeout(() => loadingEl.remove(), 500);
}

console.log(`%c[Luna] Quartz phase: ${(performance.now() - quartzStart).toFixed(0)}ms`, "color: #31d8ff; font-weight: bold;");
