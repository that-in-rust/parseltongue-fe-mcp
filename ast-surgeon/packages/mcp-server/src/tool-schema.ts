/**
 * MCP tool schema definition for fe_surgeon.
 */

export const FE_SURGEON_TOOL = {
  name: "fe_surgeon",
  description: `Apply structured AST operations to source files instead of rewriting them.

Operations: rename_symbol, add_import, remove_import, update_import_paths,
add_parameter, remove_parameter, make_async, wrap_in_block, extract_to_variable.

Faster and safer than text rewriting -- output always parses correctly.
Preserves formatting, comments, and whitespace in untouched regions.

Use dry_run: true to preview changes before applying.`,
  inputSchema: {
    type: "object" as const,
    required: ["operations"],
    properties: {
      operations: {
        type: "array" as const,
        description: "Array of operations to apply",
        items: {
          type: "object" as const,
          required: ["op"],
          properties: {
            op: {
              type: "string" as const,
              enum: [
                "rename_symbol",
                "add_import",
                "remove_import",
                "update_import_paths",
                "add_parameter",
                "remove_parameter",
                "make_async",
                "wrap_in_block",
                "extract_to_variable",
              ],
              description: "The operation type",
            },
            file: {
              type: "string" as const,
              description:
                "File path (relative to project root). Required for file-system operations.",
            },
            // rename_symbol
            from: {
              type: "string" as const,
              description: "Current symbol name (rename_symbol)",
            },
            to: {
              type: "string" as const,
              description: "New symbol name (rename_symbol)",
            },
            scope: {
              type: "string" as const,
              description:
                "Restrict rename to a scope (function/class name). Omit for entire file.",
            },
            // add_import / remove_import
            source: {
              type: "string" as const,
              description:
                'Module specifier, e.g. "react" or "./utils" (add_import, remove_import)',
            },
            specifiers: {
              type: "array" as const,
              items: { type: "string" as const },
              description:
                'Named imports, e.g. ["useState", "useEffect"] (add_import, remove_import)',
            },
            default_import: {
              type: "string" as const,
              description:
                'Default import name, e.g. "React" (add_import)',
            },
            type_only: {
              type: "boolean" as const,
              description:
                "If true, generates `import type { ... }` (add_import)",
            },
            // update_import_paths
            old_path: {
              type: "string" as const,
              description: "Old module path (update_import_paths)",
            },
            new_path: {
              type: "string" as const,
              description: "New module path (update_import_paths)",
            },
            match_mode: {
              type: "string" as const,
              enum: ["exact", "prefix"],
              description:
                'How to match: "exact" or "prefix" (update_import_paths)',
            },
            // add_parameter / remove_parameter
            function_name: {
              type: "string" as const,
              description:
                "Target function name (add_parameter, remove_parameter, make_async)",
            },
            param_name: {
              type: "string" as const,
              description:
                "Parameter name (add_parameter, remove_parameter)",
            },
            param_type: {
              type: "string" as const,
              description:
                "TypeScript type annotation (add_parameter)",
            },
            default_value: {
              type: "string" as const,
              description: "Default value expression (add_parameter)",
            },
            position: {
              type: "string" as const,
              description:
                'Where to insert: "first", "last", or 0-based index (add_parameter)',
            },
            // wrap_in_block
            start_line: {
              type: "number" as const,
              description: "First line to wrap, 1-indexed (wrap_in_block)",
            },
            end_line: {
              type: "number" as const,
              description:
                "Last line to wrap, 1-indexed inclusive (wrap_in_block)",
            },
            wrap_kind: {
              type: "string" as const,
              enum: ["if", "try_catch", "for_of", "block"],
              description:
                "Type of wrapper (wrap_in_block)",
            },
            condition: {
              type: "string" as const,
              description:
                "Condition for if, or catch param for try_catch (wrap_in_block)",
            },
            item: {
              type: "string" as const,
              description:
                "Iteration variable for for_of (wrap_in_block)",
            },
            iterable: {
              type: "string" as const,
              description:
                "Iterable expression for for_of (wrap_in_block)",
            },
            // extract_to_variable
            expression: {
              type: "string" as const,
              description:
                "The exact expression text to extract (extract_to_variable)",
            },
            variable_name: {
              type: "string" as const,
              description:
                "Name for the new variable (extract_to_variable)",
            },
            var_kind: {
              type: "string" as const,
              enum: ["const", "let"],
              description:
                '"const" or "let" (extract_to_variable)',
            },
            type_annotation: {
              type: "string" as const,
              description:
                "Optional TypeScript type annotation (extract_to_variable)",
            },
          },
        },
      },
      dry_run: {
        type: "boolean" as const,
        description:
          "If true, preview changes without modifying files. Returns what would change.",
        default: false,
      },
      project_root: {
        type: "string" as const,
        description:
          "Project root directory. File paths in operations are relative to this.",
      },
    },
  },
} as const;
