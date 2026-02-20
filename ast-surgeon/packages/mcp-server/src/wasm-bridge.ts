/**
 * WASM bridge: loads the ast-surgeon WASM module and provides
 * typed wrappers for the Rust entry points.
 */

import { createRequire } from "module";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

// Types matching the Rust protocol types

export interface SingleFileRequest {
  content: string;
  language: string;
  operations: Operation[];
  dry_run?: boolean;
}

export interface SingleFileResponse {
  error: boolean;
  content: string | null;
  changes: ChangeDescription[];
  warnings: string[];
  operation_errors: OperationErrorDetail[];
  edit_count?: number;
  status: "applied" | "preview" | "error";
}

export interface BatchRequest {
  files: BatchFileEntry[];
  dry_run?: boolean;
}

export interface BatchFileEntry {
  path: string;
  content: string;
  language: string;
  operations: Operation[];
}

export interface BatchResponse {
  results: BatchFileResult[];
  errors: BatchFileError[];
  total_edits: number;
  status: "applied" | "preview" | "partial" | "error";
}

export interface BatchFileResult {
  path: string;
  content: string;
  changes: ChangeDescription[];
  warnings: string[];
  edits_applied: number;
}

export interface BatchFileError {
  path: string;
  error: string;
  code: string;
}

export interface ChangeDescription {
  kind: string;
  line: number;
  column: number;
  summary: string;
}

export interface OperationErrorDetail {
  operation_index: number;
  code: string;
  message: string;
}

export interface Operation {
  op: string;
  file?: string;
  [key: string]: unknown;
}

interface WasmModule {
  process_file(request_json: string): string;
  process_batch(request_json: string): string;
}

let wasmModule: WasmModule | null = null;

/**
 * Load the WASM module. Call this once at startup.
 */
export async function loadWasm(): Promise<void> {
  if (wasmModule) return;

  const __filename = fileURLToPath(import.meta.url);
  const __dirname = dirname(__filename);
  const wasmPath = join(__dirname, "..", "wasm");

  // Use createRequire for CommonJS-style WASM loading from wasm-pack
  const require = createRequire(import.meta.url);
  const wasm = require(join(wasmPath, "ast_surgeon_wasm.js"));

  wasmModule = wasm;
}

/**
 * Process a single file through the WASM engine.
 */
export function processFile(request: SingleFileRequest): SingleFileResponse {
  if (!wasmModule) {
    throw new Error("WASM module not loaded. Call loadWasm() first.");
  }

  const json = wasmModule.process_file(JSON.stringify(request));
  return JSON.parse(json);
}

/**
 * Process multiple files in a batch through the WASM engine.
 */
export function processBatch(request: BatchRequest): BatchResponse {
  if (!wasmModule) {
    throw new Error("WASM module not loaded. Call loadWasm() first.");
  }

  const json = wasmModule.process_batch(JSON.stringify(request));
  return JSON.parse(json);
}
