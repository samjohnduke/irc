import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

const guide = defineCollection({
  loader: glob({ pattern: "**/*.mdx", base: "./src/content/guide" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    order: z.number(),
    section: z.string(),
  }),
});

export const collections = { guide };
