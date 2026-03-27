// Always expose internals first
export { tidalModules } from "./exposeTidalInternals";
export { buildActions, interceptors } from "./exposeTidalInternals.patchAction";

export * as ftch from "./helpers/fetch";
export { findModuleByProperty, findModuleProperty, recursiveSearch } from "./helpers/findModule";
export { unloadSet, type LunaUnload, type LunaUnloads, type NullishLunaUnloads } from "./helpers/unloadSet";

export { Messager, Tracer } from "./trace";

export { modules, reduxStore } from "./modules";

export * from "./LunaPlugin";
export * from "./ReactiveStore";
export * from "./SettingsTransfer";

// Ensure this is loaded
import "./window.core";

import { LunaPlugin } from "./LunaPlugin";
import { applySeedSettings } from "./helpers/applySeedSettingsJSOn";

type TimingEntry = { label: string; duration: number };
const timings: TimingEntry[] = [];

const timed = async (label: string, fn: () => Promise<void>) => {
	const start = performance.now();
	await fn();
	const duration = performance.now() - start;
	timings.push({ label, duration });
};

const printTimings = () => {
	const total = timings.reduce((sum, t) => sum + t.duration, 0);
	const sorted = [...timings].sort((a, b) => b.duration - a.duration);

	console.group(`%c[Luna] Startup complete in ${total.toFixed(0)}ms`, "color: #31d8ff; font-weight: bold;");
	console.log("%cBreakdown (slowest first):", "color: #a7a7a9;");
	for (const { label, duration } of sorted) {
		const pct = ((duration / total) * 100).toFixed(1);
		const bar = "█".repeat(Math.round(Number(pct) / 5));
		const color = duration > 1000 ? "color: red;" : duration > 500 ? "color: orange;" : "color: green;";
		console.log(`%c${bar} ${label}: ${duration.toFixed(0)}ms (${pct}%)`, color);
	}
	console.groupEnd();
};

// Wrap loading of plugins in a timeout so native/preload.ts can populate modules with @luna/core (see native/preload.ts)
setTimeout(async () => {
	const totalStart = performance.now();

	await timed("lib.native", () => LunaPlugin.fromStorage({ enabled: true, url: "https://luna/luna.lib.native" }));
	await timed("lib", () => LunaPlugin.fromStorage({ enabled: true, url: "https://luna/luna.lib" }));

	if (__platform === "linux") {
		await timed("linux", () => LunaPlugin.fromStorage({ enabled: true, url: "https://luna/luna.linux" }));
	}

	await timed("ui", () => LunaPlugin.fromStorage({ enabled: true, url: "https://luna/luna.ui" }));
	await timed("dev", () => LunaPlugin.fromStorage({ enabled: true, url: "https://luna/luna.dev" }));
	await timed("applySeedSettings", () => applySeedSettings());
	await timed("prefetchAll", () => LunaPlugin.pluginStorage.prefetchAll());

	// Time each user plugin individually
	const keys = await LunaPlugin.pluginStorage.keys();
	await timed("loadStoredPlugins (total)", () => LunaPlugin.loadStoredPlugins());

	printTimings();
	console.log(`%c[Luna] Total wall time: ${(performance.now() - totalStart).toFixed(0)}ms`, "color: #31d8ff;");
});
