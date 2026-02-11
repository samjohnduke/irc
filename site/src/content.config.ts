import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

const userGuide = defineCollection({
  loader: glob({ pattern: "**/*.mdx", base: "./src/content/user-guide" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    order: z.number(),
    section: z.string(),
  }),
});

const serverGuide = defineCollection({
  loader: glob({ pattern: "**/*.mdx", base: "./src/content/server-guide" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    order: z.number(),
    section: z.string(),
  }),
});

const devGuide = defineCollection({
  loader: glob({ pattern: "**/*.mdx", base: "./src/content/dev-guide" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    order: z.number(),
    section: z.string(),
  }),
});

export const collections = { "user-guide": userGuide, "server-guide": serverGuide, "dev-guide": devGuide };
