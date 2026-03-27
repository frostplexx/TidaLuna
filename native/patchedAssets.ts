import { createHash } from "crypto";
import { mkdir, readFile, writeFile, readdir, rm } from "fs/promises";
import { existsSync } from "fs";
import path from "path";
import { app } from "electron";

const patchedDir = path.join(app.getPath("userData"), "luna-patched-assets");

// Ensure patched dir exists
await mkdir(patchedDir, { recursive: true });

type AssetManifest = Record<string, string>; // path -> hash

const manifestPath = path.join(patchedDir, "manifest.json");

const loadManifest = async (): Promise<AssetManifest> => {
	try {
		return JSON.parse(await readFile(manifestPath, "utf8"));
	} catch {
		return {};
	}
};

const saveManifest = async (manifest: AssetManifest) => {
	await writeFile(manifestPath, JSON.stringify(manifest));
};

// Mirror of findCreateActionFunction from render side
const findCreateActionFunction = (code: string): { fnName: string; startIdx: number } | null => {
	const payloadMetaMatch = code.match(/\.payload,\.{3}(?:"|'|`)meta(?:"|'|`)in /);
	if (!payloadMetaMatch) return null;
	const payloadMetaIndex = payloadMetaMatch.index!;

	const codeBeforePattern = code.slice(0, payloadMetaIndex);
	const functionStartIndex = codeBeforePattern.lastIndexOf("{function");
	if (functionStartIndex === -1) return null;

	const codeBeforeFunction = code.slice(0, functionStartIndex);
	const openParenIndex = codeBeforeFunction.lastIndexOf("(");
	if (openParenIndex === -1) return null;

	const codeBeforeParen = code.slice(0, openParenIndex);
	const spaceIndex = codeBeforeParen.lastIndexOf(" ");

	const startIdx = spaceIndex + 1;
	const fnName = code.substring(startIdx, openParenIndex).trim();

	if (!fnName) return null;
	return { fnName, startIdx };
};

export const transformCode = (code: string): string => {
	const actionData = findCreateActionFunction(code);
	if (!actionData) return code;

	const { fnName, startIdx } = actionData;
	const funcPrefix = "__LunaUnpatched_";
	const renamedFn = funcPrefix + fnName;

	let transformed = code.slice(0, startIdx) + funcPrefix + code.slice(startIdx);

	const declarationStartIdx = startIdx - 9;
	const patchedDeclaration = `const ${fnName} = patchAction({ _: ${renamedFn} })._;`;
	transformed = transformed.slice(0, declarationStartIdx) + patchedDeclaration + transformed.slice(declarationStartIdx);

	return transformed;
};

export const getPatchedPath = (urlPath: string): string => {
	// Convert /assets/index-Doo9tDW7.js to a safe filename
	const safe = urlPath.replace(/\//g, "_").replace(/^_/, "");
	return path.join(patchedDir, safe);
};

export const hasPatchedAsset = async (urlPath: string, etag: string | null): Promise<boolean> => {
	if (!etag) return false;
	const manifest = await loadManifest();
	return manifest[urlPath] === etag && existsSync(getPatchedPath(urlPath));
};

export const getPatchedAsset = async (urlPath: string): Promise<Buffer | null> => {
	try {
		return await readFile(getPatchedPath(urlPath));
	} catch {
		return null;
	}
};

export const patchAndSaveAsset = async (urlPath: string, code: string, etag: string | null): Promise<string> => {
	// Only bother transforming files large enough to contain createAction
	const transformed = code.length > 1000 ? transformCode(code) : code;
	await writeFile(getPatchedPath(urlPath), transformed, "utf8");

	if (etag) {
		const manifest = await loadManifest();
		manifest[urlPath] = etag;
		await saveManifest(manifest);
	}

	return transformed;
};

// Clear patched assets that no longer match manifest (cleanup on Tidal update)
export const clearStaleAssets = async (validPaths: string[]) => {
	const manifest = await loadManifest();
	const validSafePaths = new Set(validPaths.map((p) => p.replace(/\//g, "_").replace(/^_/, "")));

	try {
		const files = await readdir(patchedDir);
		for (const file of files) {
			if (file === "manifest.json") continue;
			if (!validSafePaths.has(file)) {
				await rm(path.join(patchedDir, file)).catch(() => {});
			}
		}
	} catch {}
};
