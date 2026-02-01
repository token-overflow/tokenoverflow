import { type InferOutput, object, picklist, pipe, string, url } from "valibot";

export const AppConfigSchema = object({
  env: picklist(["local", "production"]),
  landing: object({
    base_url: pipe(string(), url()),
  }),
});

export type AppConfig = InferOutput<typeof AppConfigSchema>;
