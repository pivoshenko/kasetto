import { Callout } from "@/app/components/callout";
import { Mermaid } from "@/app/components/mermaid";
import defaultMdxComponents from "fumadocs-ui/mdx";
import type { MDXComponents } from "mdx/types";

export function getMDXComponents(components?: MDXComponents): MDXComponents {
  return {
    ...defaultMdxComponents,
    Callout,
    Mermaid,
    ...components,
  };
}
