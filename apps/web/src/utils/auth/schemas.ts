import { type InferOutput, literal, object, optional, safeParse, string, union } from "valibot";

/// Callback query: success path carries `code` + `state`, denial path
/// carries `error` + `state`. Strict union so unrecognised shapes fail
/// closed.
export const CallbackQuerySuccessSchema = object({
  code: string(),
  state: string(),
});

export const CallbackQueryErrorSchema = object({
  error: string(),
  state: string(),
  error_description: optional(string()),
});

export const CallbackQuerySchema = union([CallbackQuerySuccessSchema, CallbackQueryErrorSchema]);

export type CallbackQuery =
  | { code: string; state: string }
  | { error: string; state: string; error_description?: string };

/// Reserved literal: the `intent` query/cookie field must be `waitlist`.
/// Other intents will be added in subsequent designs.
export const StartIntentSchema = literal("waitlist");

/// Inferred valibot output for the callback query schema. Slightly
/// different from `CallbackQuery` (e.g. `error_description?: string |
/// undefined` vs `error_description?: string`); we expose the inferred
/// shape through the parser so callers do not need a cast.
type CallbackQueryOutput = InferOutput<typeof CallbackQuerySchema>;

export type ParsedCallbackQuery = { ok: true; query: CallbackQueryOutput } | { ok: false };

/// Parse the callback query string into a `Result`. Hides the
/// `safeParse` shape from callers so the route handler stays focused on
/// the OAuth flow rather than schema plumbing.
export function parseCallbackQuery(query_object: Record<string, string>): ParsedCallbackQuery {
  const parsed = safeParse(CallbackQuerySchema, query_object);
  return parsed.success ? { ok: true, query: parsed.output } : { ok: false };
}
