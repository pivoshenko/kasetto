import { defineConfig, defineDocs } from "fumadocs-mdx/config";
import type { Code, Root } from "mdast";
import type { Plugin } from "unified";
import { visit } from "unist-util-visit";

export const docs = defineDocs({
  dir: "content/docs",
});

// Convert ```mermaid code blocks into <Mermaid chart={...} /> MDX JSX nodes
// so they bypass Shiki and render as live diagrams on the client.
const remarkMermaid: Plugin<[], Root> = () => (tree) => {
  visit(tree, "code", (node: Code, index, parent) => {
    if (node.lang !== "mermaid" || !parent || index === undefined) return;
    parent.children[index] = {
      type: "mdxJsxFlowElement",
      name: "Mermaid",
      attributes: [
        {
          type: "mdxJsxAttribute",
          name: "chart",
          value: node.value,
        },
      ],
      children: [] as never,
    } as never;
  });
};

export default defineConfig({
  mdxOptions: {
    remarkPlugins: [remarkMermaid],
    rehypeCodeOptions: {
      themes: {
        light: "catppuccin-latte",
        dark: "catppuccin-mocha",
      },
    },
  },
});
