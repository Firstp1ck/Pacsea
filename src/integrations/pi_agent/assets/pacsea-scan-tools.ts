// Pacsea AUR scanner restricted-tool extension (contract: pacsea-scan-tools-1).
//
// This file is compiled into the Pacsea binary, materialized atomically at mode 0600
// inside a private mode-0700 runtime directory, and its SHA-256 is re-verified from
// disk immediately before Pi is launched. Editing it changes the compiled asset hash.
//
// Security contract mirrored from `restricted_tools.rs`:
//   * only four tools exist: read, grep, find, ls; all are read-only;
//   * snapshot roots come from a private sibling descriptor, never from model input;
//   * paths must be relative, normalized, depth-bounded, and control-free;
//   * containment is checked after realpath, so symlink escapes are rejected;
//   * grep is literal substring search only, never a regular expression;
//   * every request bound and result bound is enforced, and oversized requests are
//     rejected rather than clamped;
//   * no shell, write, edit, network, process, environment, or UI capability is exposed.

import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";
import { constants } from "node:fs";
import { readFile, readdir, realpath, stat, lstat, open, type FileHandle } from "node:fs/promises";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

/** Fixed name of the private snapshot descriptor written next to this extension. */
const DESCRIPTOR_FILE_NAME = "pacsea-scan-descriptor.json";

/** Compiled hard maxima; these mirror `limits` in the Rust bridge exactly. */
const LIMITS = {
  analyzableTextBytes: 16 * 1024 * 1024,
  readBytes: 64 * 1024,
  grepMatches: 200,
  grepBytes: 128 * 1024,
  listingEntries: 500,
  listingBytes: 128 * 1024,
  pathDepth: 16,
  grepLineChars: 512,
  literalChars: 1024,
  globChars: 256,
  walkVisits: 10000,
} as const;

/** Snapshot id to absolute canonical root, loaded once from the private descriptor. */
type SnapshotRoots = ReadonlyMap<string, string>;

/** Rejection carrying inert, disclosure-free text for the model. */
class ToolRejection extends Error {}

/** Reject a request with inert wording that never echoes host paths or file bytes. */
function reject(message: string): never {
  throw new ToolRejection(message);
}

/** Load the private descriptor that maps opaque snapshot ids to absolute roots. */
async function loadSnapshotRoots(): Promise<SnapshotRoots> {
  const here = dirname(fileURLToPath(import.meta.url));
  const raw = await readFile(join(here, DESCRIPTOR_FILE_NAME), "utf8");
  const parsed: unknown = JSON.parse(raw);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    reject("the snapshot descriptor is malformed");
  }
  const entries = Object.entries(parsed as Record<string, unknown>);
  const roots = new Map<string, string>();
  for (const [id, value] of entries) {
    if (typeof value !== "string" || value.length === 0) {
      reject("the snapshot descriptor is malformed");
    }
    roots.set(id, await realpath(value));
  }
  return roots;
}

/** Resolve a snapshot id to its canonical root, re-verifying it on every call. */
async function snapshotRoot(roots: SnapshotRoots, snapshot: string): Promise<string> {
  const recorded = roots.get(snapshot);
  if (recorded === undefined) {
    reject(`unknown snapshot ${JSON.stringify(snapshot)}`);
  }
  let current: string;
  try {
    current = await realpath(recorded);
  } catch {
    reject(`snapshot ${JSON.stringify(snapshot)} is no longer available`);
  }
  if (current !== recorded || !(await stat(current)).isDirectory()) {
    reject(`snapshot ${JSON.stringify(snapshot)} is no longer the directory Pacsea prepared`);
  }
  return recorded;
}

/** Reject control characters, Unicode separators, and Windows-style separators. */
function hasForbiddenControl(text: string): boolean {
  for (const character of text) {
    const code = character.codePointAt(0) ?? 0;
    if (code < 0x20 || code === 0x7f || (code >= 0x80 && code <= 0x9f)) return true;
    if (code === 0x2028 || code === 0x2029) return true;
  }
  return false;
}

/** Validate a model-supplied relative path without touching the filesystem. */
function validateRelativePath(relative: string): string {
  if (relative.length === 0) reject("the path must not be empty");
  if (hasForbiddenControl(relative) || relative.includes("\\")) {
    reject("path components must not contain control characters or separators");
  }
  if (relative.startsWith("/")) reject("absolute paths are not allowed");
  if (/^[A-Za-z]:/.test(relative)) reject("absolute paths are not allowed");
  const parts = relative.split("/");
  if (parts.some((part) => part.length === 0)) reject("the path must be normalized");
  if (parts.length > LIMITS.pathDepth) {
    reject(`paths may not be deeper than ${LIMITS.pathDepth} components`);
  }
  for (const part of parts) {
    if (part === "." || part === "..") reject("'.' and '..' path components are not allowed");
  }
  return parts.join("/");
}

/** Return the procfs path for one already-open descriptor. */
function descriptorPath(handle: FileHandle): string {
  return `/proc/self/fd/${handle.fd}`;
}

/** Verify that an opened descriptor still names an object inside the recorded root. */
async function verifyOpenedDescriptor(
  root: string,
  handle: FileHandle,
  expectDirectory: boolean,
): Promise<void> {
  let canonical: string;
  try {
    canonical = await realpath(descriptorPath(handle));
  } catch {
    await handle.close();
    reject("the opened snapshot entry could not be verified");
  }
  if (canonical !== root && !canonical.startsWith(root + sep)) {
    await handle.close();
    reject("the path resolves outside the snapshot");
  }
  const info = await handle.stat();
  if (expectDirectory ? !info.isDirectory() : !info.isFile()) {
    await handle.close();
    reject("only regular files and directories can be accessed");
  }
}

/** Open and pin one snapshot entry before checking containment, defeating path-swap races. */
async function openPinnedEntry(
  root: string,
  relative: string | undefined,
  expectDirectory: boolean,
): Promise<FileHandle> {
  const target = relative === undefined ? root : resolve(root, validateRelativePath(relative));
  const typeFlag = expectDirectory ? constants.O_DIRECTORY : 0;
  let handle: FileHandle;
  try {
    handle = await open(target, constants.O_RDONLY | constants.O_NOFOLLOW | typeFlag);
  } catch {
    reject("no such entry in this snapshot");
  }
  await verifyOpenedDescriptor(root, handle, expectDirectory);
  return handle;
}

/** Validate an optional numeric bound, rejecting zero and anything above the maximum. */
function checkBound(parameter: string, requested: number | undefined, maximum: number): number {
  if (requested === undefined) return maximum;
  if (!Number.isInteger(requested) || requested <= 0) {
    reject(`${parameter} must be a positive integer`);
  }
  if (requested > maximum) reject(`${parameter} ${requested} exceeds the limit of ${maximum}`);
  return requested;
}

/** Replace control characters so hostile source cannot emit terminal escapes. */
function boundedLine(line: string): string {
  let out = "";
  for (const character of line) {
    if (out.length >= LIMITS.grepLineChars) break;
    const code = character.codePointAt(0) ?? 0;
    out += code < 0x20 || code === 0x7f ? "\ufffd" : character;
  }
  return out;
}

/** Bounded deterministic file-walk result. */
type WalkResult = { files: string[]; truncated: boolean };

/** Enumerate regular files under a root without following symlinks. */
async function walkFiles(root: string): Promise<WalkResult> {
  const found: string[] = [];
  const queue: Array<{ relative?: string; prefix: string; depth: number }> = [
    { prefix: "", depth: 0 },
  ];
  let visited = 0;
  let truncated = false;
  while (queue.length > 0) {
    const current = queue.pop();
    if (current === undefined) continue;
    if (current.depth > LIMITS.pathDepth) {
      truncated = true;
      continue;
    }
    let handle: FileHandle;
    try {
      handle = await openPinnedEntry(root, current.relative, true);
    } catch {
      continue;
    }
    try {
      const pinnedDirectory = descriptorPath(handle);
      const items = await readdir(pinnedDirectory);
      for (const name of items) {
        if (++visited >= LIMITS.walkVisits) {
          truncated = true;
          break;
        }
        const relative = current.prefix.length === 0 ? name : `${current.prefix}/${name}`;
        let info: Awaited<ReturnType<typeof lstat>>;
        try {
          info = await lstat(join(pinnedDirectory, name));
        } catch {
          continue;
        }
        if (info.isDirectory()) {
          queue.push({ relative, prefix: relative, depth: current.depth + 1 });
        } else if (info.isFile()) {
          found.push(relative);
        }
      }
    } finally {
      await handle.close();
    }
    if (truncated) break;
  }
  return { files: found.sort(), truncated };
}

/** Match a bounded glob (`*`, `**`, `?`) against a snapshot-relative path. */
function globMatches(pattern: string, path: string): boolean {
  const p = [...pattern];
  const t = [...path];
  const matched = Array.from({ length: p.length + 1 }, () =>
    Array.from({ length: t.length + 1 }, () => false),
  );
  matched[p.length]![t.length] = true;
  for (let pi = p.length - 1; pi >= 0; pi -= 1) {
    for (let ti = t.length; ti >= 0; ti -= 1) {
      if (p[pi] === "*") {
        const crosses = p[pi + 1] === "*";
        const next = pi + (crosses ? 2 : 1);
        const consumes =
          ti < t.length &&
          (crosses || t[ti] !== "/") &&
          matched[pi]![ti + 1] === true;
        matched[pi]![ti] = matched[next]![ti] === true || consumes;
      } else if (ti < t.length) {
        const consumes = p[pi] === t[ti] || (p[pi] === "?" && t[ti] !== "/");
        matched[pi]![ti] = consumes && matched[pi + 1]![ti + 1] === true;
      }
    }
  }
  return matched[0]![0] === true;
}

/** Wrap a JSON payload in the text content shape Pi expects from a tool. */
function textResult(payload: unknown) {
  return {
    content: [{ type: "text" as const, text: JSON.stringify(payload) }],
    details: {},
  };
}

/** Register the four path-confined read-only scanner tools. */
export default function pacseaScanTools(pi: ExtensionAPI) {
  let rootsPromise: Promise<SnapshotRoots> | undefined;
  const roots = () => (rootsPromise ??= loadSnapshotRoots());

  /** Run a tool body and convert rejections into inert model-visible errors. */
  const guarded = async (body: () => Promise<unknown>) => {
    try {
      return textResult(await body());
    } catch (error) {
      const message = error instanceof ToolRejection ? error.message : "the request was rejected";
      return textResult({ error: message });
    }
  };

  pi.registerTool({
    name: "pacsea_scan_read",
    label: "pacsea_scan_read",
    description:
      "Read a bounded UTF-8 window from one file inside an approved Pacsea snapshot. " +
      "Paths are snapshot-relative; absolute paths and '..' are rejected.",
    parameters: Type.Object({
      snapshot: Type.String(),
      relative_path: Type.String(),
      offset: Type.Optional(Type.Integer({ minimum: 0 })),
      limit: Type.Optional(Type.Integer({ minimum: 1, maximum: LIMITS.readBytes })),
    }),
    async execute(args: {
      snapshot: string;
      relative_path: string;
      offset?: number;
      limit?: number;
    }) {
      return guarded(async () => {
        const limit = checkBound("limit", args.limit, LIMITS.readBytes);
        const offset = args.offset ?? 0;
        if (!Number.isSafeInteger(offset) || offset < 0) {
          reject("offset must be a non-negative safe integer");
        }
        const root = await snapshotRoot(await roots(), args.snapshot);
        const handle = await openPinnedEntry(root, args.relative_path, false);
        try {
          const info = await handle.stat();
          const buffer = Buffer.alloc(limit);
          const { bytesRead } = await handle.read(buffer, 0, limit, offset);
          const window = buffer.subarray(0, bytesRead);
          const truncated = offset + bytesRead < info.size;
          const decoder = new TextDecoder("utf-8", { fatal: true });
          let text: string;
          try {
            text = decoder.decode(window, { stream: truncated });
          } catch {
            reject("this file is not valid UTF-8 text and cannot be read as text");
          }
          return {
            path: args.relative_path,
            offset,
            text,
            total_bytes: info.size,
            truncated,
          };
        } finally {
          await handle.close();
        }
      });
    },
  });

  pi.registerTool({
    name: "pacsea_scan_grep",
    label: "pacsea_scan_grep",
    description:
      "Bounded literal substring search inside an approved Pacsea snapshot. " +
      "The pattern is a literal string; regular expressions are not supported.",
    parameters: Type.Object({
      snapshot: Type.String(),
      literal: Type.String({ minLength: 1, maxLength: LIMITS.literalChars }),
      case_sensitive: Type.Optional(Type.Boolean()),
      max_matches: Type.Optional(Type.Integer({ minimum: 1, maximum: LIMITS.grepMatches })),
    }),
    async execute(args: {
      snapshot: string;
      literal: string;
      case_sensitive?: boolean;
      max_matches?: number;
    }) {
      return guarded(async () => {
        const bound = checkBound("max_matches", args.max_matches, LIMITS.grepMatches);
        if (args.literal.length === 0) reject("the search literal must not be empty");
        if (args.literal.length > LIMITS.literalChars) {
          reject(`literal exceeds the limit of ${LIMITS.literalChars}`);
        }
        const caseSensitive = args.case_sensitive ?? true;
        const needle = caseSensitive ? args.literal : args.literal.toLowerCase();
        const root = await snapshotRoot(await roots(), args.snapshot);
        const matches: Array<{ path: string; line: number; text: string }> = [];
        let budget = LIMITS.grepBytes;
        const walked = await walkFiles(root);
        let truncated = walked.truncated;
        for (const relative of walked.files) {
          if (matches.length >= bound) {
            truncated = true;
            break;
          }
          let content: string;
          let handle: FileHandle;
          try {
            handle = await openPinnedEntry(root, relative, false);
          } catch {
            continue;
          }
          try {
            const info = await handle.stat();
            if (info.size > LIMITS.analyzableTextBytes) continue;
            const bytes = await handle.readFile();
            content = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
          } catch {
            continue;
          } finally {
            await handle.close();
          }
          const lines = content.split("\n");
          for (let index = 0; index < lines.length; index += 1) {
            if (matches.length >= bound) {
              truncated = true;
              break;
            }
            const line = lines[index] ?? "";
            const haystack = caseSensitive ? line : line.toLowerCase();
            if (!haystack.includes(needle)) continue;
            const text = boundedLine(line);
            const cost = text.length + relative.length + 16;
            if (cost > budget) {
              truncated = true;
              break;
            }
            budget -= cost;
            matches.push({ path: relative, line: index + 1, text });
          }
          if (truncated) break;
        }
        return { matches, truncated };
      });
    },
  });

  pi.registerTool({
    name: "pacsea_scan_find",
    label: "pacsea_scan_find",
    description:
      "Find files inside an approved Pacsea snapshot with a bounded glob using '*', '**', and '?'.",
    parameters: Type.Object({
      snapshot: Type.String(),
      glob: Type.String({ minLength: 1, maxLength: LIMITS.globChars }),
      max_results: Type.Optional(Type.Integer({ minimum: 1, maximum: LIMITS.listingEntries })),
    }),
    async execute(args: { snapshot: string; glob: string; max_results?: number }) {
      return guarded(async () => {
        const bound = checkBound("max_results", args.max_results, LIMITS.listingEntries);
        if (args.glob.length === 0) reject("the glob must not be empty");
        if (args.glob.length > LIMITS.globChars) {
          reject(`glob exceeds the limit of ${LIMITS.globChars}`);
        }
        if (hasForbiddenControl(args.glob)) {
          reject("path components must not contain control characters or separators");
        }
        const root = await snapshotRoot(await roots(), args.snapshot);
        const entries: Array<{ path: string; kind: string; size: number }> = [];
        let budget = LIMITS.listingBytes;
        const walked = await walkFiles(root);
        let truncated = walked.truncated;
        for (const relative of walked.files) {
          if (entries.length >= bound) {
            truncated = true;
            break;
          }
          if (!globMatches(args.glob, relative)) continue;
          if (relative.length + 32 > budget) {
            truncated = true;
            break;
          }
          budget -= relative.length + 32;
          let handle: FileHandle;
          try {
            handle = await openPinnedEntry(root, relative, false);
          } catch {
            continue;
          }
          try {
            const info = await handle.stat();
            entries.push({ path: relative, kind: "file", size: info.size });
          } finally {
            await handle.close();
          }
        }
        entries.sort((left, right) => (left.path < right.path ? -1 : left.path > right.path ? 1 : 0));
        return { entries, truncated };
      });
    },
  });

  pi.registerTool({
    name: "pacsea_scan_ls",
    label: "pacsea_scan_ls",
    description:
      "List one directory inside an approved Pacsea snapshot. Symlinks are reported as inert metadata and are never followed.",
    parameters: Type.Object({
      snapshot: Type.String(),
      relative_path: Type.Optional(Type.String()),
      max_entries: Type.Optional(Type.Integer({ minimum: 1, maximum: LIMITS.listingEntries })),
    }),
    async execute(args: { snapshot: string; relative_path?: string; max_entries?: number }) {
      return guarded(async () => {
        const bound = checkBound("max_entries", args.max_entries, LIMITS.listingEntries);
        const root = await snapshotRoot(await roots(), args.snapshot);
        const handle = await openPinnedEntry(root, args.relative_path, true);
        try {
          const prefix = (args.relative_path ?? "").replace(/\/+$/u, "");
          const pinnedDirectory = descriptorPath(handle);
          const names = (await readdir(pinnedDirectory)).sort();
          const entries: Array<{ path: string; kind: string; size: number }> = [];
          let budget = LIMITS.listingBytes;
          let truncated = false;
          for (const name of names) {
            if (entries.length >= bound) {
              truncated = true;
              break;
            }
            const relative = prefix.length === 0 ? name : `${prefix}/${name}`;
            if (relative.length + 32 > budget) {
              truncated = true;
              break;
            }
            budget -= relative.length + 32;
            const info = await lstat(join(pinnedDirectory, name)).catch(() => undefined);
            entries.push({
              path: relative,
              kind: info?.isFile() === true ? "file" : info?.isDirectory() === true ? "dir" : "other",
              size: info?.isFile() === true ? info.size : 0,
            });
          }
          entries.sort((left, right) =>
            left.path < right.path ? -1 : left.path > right.path ? 1 : 0,
          );
          return { entries, truncated };
        } finally {
          await handle.close();
        }
      });
    },
  });

  pi.registerCommand("pacsea-scan-tools", {
    description: "Report the exact active tool allowlist without invoking a model",
    handler: async (_args: unknown, ctx: { ui: { notify: (m: string, l: string) => void } }) => {
      const active = [...pi.getActiveTools()].sort();
      ctx.ui.notify(`PACSEA_ACTIVE_TOOLS:${JSON.stringify(active)}`, "info");
    },
  });
}
