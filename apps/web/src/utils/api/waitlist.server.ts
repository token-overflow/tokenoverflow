import { config } from "@tokenoverflow/config";

import { callApi, type ApiResult } from "./_call.server";
import { addToWaitlist as addToWaitlistGenerated } from "./_generated/sdk.gen";

import type { AddToWaitlistResponse } from "./_generated/types.gen";

export const addToWaitlist = (opts: {
  workos_jwt: string;
}): Promise<ApiResult<AddToWaitlistResponse>> =>
  callApi((signal) =>
    addToWaitlistGenerated({
      baseUrl: config.api.base_url,
      headers: {
        authorization: `Bearer ${opts.workos_jwt}`,
        "content-type": "application/json",
      },
      body: {} as never,
      signal,
    }),
  );
