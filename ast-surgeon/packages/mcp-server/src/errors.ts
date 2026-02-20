/**
 * Structured error types for the MCP server.
 */

export class AstSurgeonError extends Error {
  constructor(
    message: string,
    public code: string,
    public details?: Record<string, unknown>
  ) {
    super(message);
    this.name = "AstSurgeonError";
  }
}

export class FileNotFoundError extends AstSurgeonError {
  constructor(path: string) {
    super(`File not found: ${path}`, "FILE_NOT_FOUND", { path });
    this.name = "FileNotFoundError";
  }
}

export class UnsupportedLanguageError extends AstSurgeonError {
  constructor(ext: string) {
    super(
      `Unsupported file extension: ${ext}`,
      "UNSUPPORTED_LANGUAGE",
      { extension: ext }
    );
    this.name = "UnsupportedLanguageError";
  }
}

export class WasmNotLoadedError extends AstSurgeonError {
  constructor() {
    super("WASM module not loaded", "WASM_NOT_LOADED");
    this.name = "WasmNotLoadedError";
  }
}

/** Map file extensions to language identifiers */
export function extensionToLanguage(ext: string): string {
  const map: Record<string, string> = {
    ".ts": "typescript",
    ".tsx": "tsx",
    ".js": "javascript",
    ".jsx": "jsx",
    ".mjs": "javascript",
    ".cjs": "javascript",
    ".css": "css",
  };
  const lang = map[ext.toLowerCase()];
  if (!lang) {
    throw new UnsupportedLanguageError(ext);
  }
  return lang;
}
