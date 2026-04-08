/**
 * fetch_url tool — the right way.
 *
 * Pipeline: HTTP GET → Mozilla Readability (article extraction) → Turndown (HTML→Markdown)
 *
 * This mirrors how Claude Code, Codex CLI, and all major agent CLIs handle web content:
 * - Readability strips navigation, ads, sidebars, scripts — extracts the main article
 * - Turndown converts the clean article HTML to Markdown (token-efficient vs raw HTML)
 * - Hard truncation at MAX_CHARS as a last resort safety net
 */
import { JSDOM } from "jsdom";
import { Readability } from "@mozilla/readability";
import TurndownService from "turndown";
import { Type } from "@mariozechner/pi-ai";
import type { AgentTool } from "@mariozechner/pi-agent-core";

const MAX_CHARS = 12_000;

const turndown = new TurndownService({
  headingStyle: "atx",
  bulletListMarker: "-",
  codeBlockStyle: "fenced",
});

// Remove these from the markdown output (not useful for LLMs)
turndown.remove(["script", "style", "nav", "footer", "aside", "iframe"]);

export const fetchUrlTool: AgentTool = {
  name: "fetch_url",
  label: "Fetch URL",
  description:
    "Fetch a URL and return its main content as clean Markdown. " +
    "Navigation, ads, and boilerplate are stripped. Use this to read web pages, docs, or articles.",
  parameters: Type.Object({
    url: Type.String({ description: "The URL to fetch" }),
  }),
  execute: async (_toolCallId, params) => {
    const { url } = params as { url: string };

    const response = await fetch(url, {
      headers: {
        "User-Agent":
          "Mozilla/5.0 (compatible; BeeHive/2.0; +https://github.com/mahaddev-x/beehive)",
        Accept: "text/html,application/xhtml+xml,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.5",
      },
      signal: AbortSignal.timeout(30_000),
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status} for ${url}`);
    }

    const contentType = response.headers.get("content-type") ?? "";
    const html = await response.text();

    let markdown: string;

    if (contentType.includes("text/html")) {
      markdown = htmlToMarkdown(html, url);
    } else if (
      contentType.includes("text/markdown") ||
      contentType.includes("text/plain")
    ) {
      // Already plain text — use as-is
      markdown = html;
    } else if (contentType.includes("application/json")) {
      try {
        markdown = JSON.stringify(JSON.parse(html), null, 2);
      } catch {
        markdown = html;
      }
    } else {
      // Fallback for other content types
      markdown = htmlToMarkdown(html, url);
    }

    const truncated =
      markdown.length > MAX_CHARS
        ? `${markdown.slice(0, MAX_CHARS)}\n\n---\n*[Content truncated at ${MAX_CHARS} chars]*`
        : markdown;

    return {
      content: [{ type: "text" as const, text: truncated }],
      details: {
        url,
        chars: truncated.length,
        truncated: markdown.length > MAX_CHARS,
      },
    };
  },
};

function htmlToMarkdown(html: string, url: string): string {
  // Parse with JSDOM
  const dom = new JSDOM(html, { url });
  const document = dom.window.document;

  // Try Mozilla Readability first — extracts the main article content
  const reader = new Readability(document);
  const article = reader.parse();

  if (article?.content && article.content.length > 200) {
    // Got a clean article — convert just the article HTML to Markdown
    const md = turndown.turndown(article.content);
    const title = article.title ? `# ${article.title}\n\n` : "";
    return title + md;
  }

  // Readability failed (no clear article) — convert the whole body
  const body = document.body?.innerHTML ?? html;
  return turndown.turndown(body);
}
