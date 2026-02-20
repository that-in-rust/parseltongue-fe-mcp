/**
 * Request handler: orchestrates file I/O + WASM processing.
 *
 * Reads files from disk, detects language, calls WASM engine,
 * writes results back to disk (unless dry_run).
 */

import { readFile, writeFile } from "fs/promises";
import { extname, resolve } from "path";
import {
  processFile,
  processBatch,
  type Operation,
  type SingleFileResponse,
  type BatchResponse,
} from "./wasm-bridge.js";
import { extensionToLanguage, FileNotFoundError } from "./errors.js";

export interface ToolInput {
  operations: Operation[];
  dry_run?: boolean;
  project_root?: string;
}

export interface ToolResult {
  status: "applied" | "preview" | "partial" | "error";
  files_modified: FileModification[];
  operations_applied: number;
  operations_failed: number;
  total_edits: number;
  warnings: string[];
  errors: OperationErrorInfo[];
  hint?: string;
}

interface FileModification {
  file: string;
  edits_applied: number;
  changes: { kind: string; line: number; summary: string }[];
}

interface OperationErrorInfo {
  operation_index: number;
  code: string;
  message: string;
  suggestion?: string;
}

/**
 * Handle an fe_surgeon tool call.
 */
export async function handleToolCall(input: ToolInput): Promise<ToolResult> {
  const projectRoot = input.project_root || process.cwd();
  const dryRun = input.dry_run ?? false;

  // Group operations by file
  const fileOps = groupByFile(input.operations, projectRoot);

  const filesModified: FileModification[] = [];
  const allWarnings: string[] = [];
  const allErrors: OperationErrorInfo[] = [];
  let totalEdits = 0;
  let opsApplied = 0;
  let opsFailed = 0;

  // Process each file
  for (const [filePath, ops] of fileOps) {
    let content: string;
    try {
      content = await readFile(filePath, "utf-8");
    } catch {
      allErrors.push({
        operation_index: -1,
        code: "FILE_NOT_FOUND",
        message: `File not found: ${filePath}`,
        suggestion: "Check the file path and ensure it exists.",
      });
      opsFailed += ops.length;
      continue;
    }

    const ext = extname(filePath);
    let language: string;
    try {
      language = extensionToLanguage(ext);
    } catch {
      allErrors.push({
        operation_index: -1,
        code: "UNSUPPORTED_LANGUAGE",
        message: `Unsupported file type: ${ext}`,
        suggestion:
          "Supported: .ts, .tsx, .js, .jsx, .mjs, .cjs, .css",
      });
      opsFailed += ops.length;
      continue;
    }

    const response: SingleFileResponse = processFile({
      content,
      language,
      operations: ops,
      dry_run: dryRun,
    });

    if (response.error) {
      for (const err of response.operation_errors) {
        allErrors.push({
          operation_index: err.operation_index,
          code: err.code,
          message: err.message,
          suggestion: errorSuggestion(err.code),
        });
      }
      opsFailed += ops.length;
      continue;
    }

    // Write back if not dry_run
    if (!dryRun && response.content) {
      await writeFile(filePath, response.content, "utf-8");
    }

    totalEdits += response.changes.length;
    opsApplied += ops.length;
    allWarnings.push(...response.warnings);

    filesModified.push({
      file: filePath,
      edits_applied: response.changes.length,
      changes: response.changes.map((c) => ({
        kind: c.kind,
        line: c.line,
        summary: c.summary,
      })),
    });
  }

  const status = allErrors.length === 0
    ? dryRun
      ? "preview"
      : "applied"
    : filesModified.length > 0
      ? "partial"
      : "error";

  return {
    status,
    files_modified: filesModified,
    operations_applied: opsApplied,
    operations_failed: opsFailed,
    total_edits: totalEdits,
    warnings: allWarnings,
    errors: allErrors,
    hint:
      status === "applied"
        ? "Call fe_verify to confirm lint/types/tests pass."
        : undefined,
  };
}

/**
 * Group operations by their target file.
 */
function groupByFile(
  operations: Operation[],
  projectRoot: string
): Map<string, Operation[]> {
  const groups = new Map<string, Operation[]>();

  for (const op of operations) {
    const file = op.file;
    if (!file) {
      // If no file specified, we can't process it
      continue;
    }
    const fullPath = resolve(projectRoot, file);
    if (!groups.has(fullPath)) {
      groups.set(fullPath, []);
    }
    groups.get(fullPath)!.push(op);
  }

  return groups;
}

/**
 * Suggest fixes for common error codes.
 */
function errorSuggestion(code: string): string | undefined {
  switch (code) {
    case "SYMBOL_NOT_FOUND":
      return "Check the symbol name. Use exact casing. The symbol must exist in the file.";
    case "AMBIGUOUS_MATCH":
      return 'Specify a "scope" to disambiguate (e.g., a function or class name).';
    case "EDIT_CONFLICT":
      return "Two operations target the same code. Split into separate calls.";
    case "INVALID_RESULT":
      return "This is a bug in ast-surgeon. Please report it. Falling back to text editing is recommended.";
    case "SOURCE_HAS_ERRORS":
      return "The source file has syntax errors. Fix them first or use text editing.";
    case "UNSUPPORTED_LANGUAGE":
      return "This file type is not supported. Supported: TypeScript, TSX, JavaScript, JSX, CSS.";
    case "INVALID_PARAMS":
      return "Check the operation parameters against the schema.";
    default:
      return undefined;
  }
}
