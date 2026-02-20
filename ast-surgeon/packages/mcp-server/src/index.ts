#!/usr/bin/env node

/**
 * ast-surgeon MCP server.
 *
 * Provides the `fe_surgeon` tool over MCP stdio transport.
 * Loads the Rust/WASM engine at startup for sub-50ms operation latency.
 */

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { loadWasm } from "./wasm-bridge.js";
import { handleToolCall, type ToolInput } from "./handler.js";
import { FE_SURGEON_TOOL } from "./tool-schema.js";

async function main() {
  // Load WASM engine
  await loadWasm();

  const server = new Server(
    {
      name: "ast-surgeon",
      version: "0.1.0",
    },
    {
      capabilities: {
        tools: {},
      },
    }
  );

  // List available tools
  server.setRequestHandler(ListToolsRequestSchema, async () => {
    return {
      tools: [FE_SURGEON_TOOL],
    };
  });

  // Handle tool calls
  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    if (request.params.name !== "fe_surgeon") {
      return {
        content: [
          {
            type: "text",
            text: `Unknown tool: ${request.params.name}`,
          },
        ],
        isError: true,
      };
    }

    try {
      const input = request.params.arguments as unknown as ToolInput;
      const result = await handleToolCall(input);

      return {
        content: [
          {
            type: "text",
            text: JSON.stringify(result, null, 2),
          },
        ],
        isError: result.status === "error",
      };
    } catch (error) {
      const message =
        error instanceof Error ? error.message : String(error);
      return {
        content: [
          {
            type: "text",
            text: JSON.stringify(
              {
                status: "error",
                message,
                hint: "Check the operation parameters and file paths.",
              },
              null,
              2
            ),
          },
        ],
        isError: true,
      };
    }
  });

  // Connect via stdio
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
