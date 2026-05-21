// The pino-lambda plugin wants the AWS request id on every log line.
// The clean source is `event.req.runtime.aws.context.awsRequestId`,
// but Nitro 3.0's aws-lambda preset rebuilds the inbound request and
// drops the `runtime.aws` context. API Gateway / ALB always set
// `x-amzn-requestid`, so read that and hand pino-lambda a synthetic
// event+context. Delete once the preset stops rebuilding the request
// (verify `event.req.runtime.aws.context` populates first).

import { randomUUID } from "node:crypto";

import { definePlugin } from "nitro";

import { withRequest } from "../../utils/logging/logger.server";

const REQUEST_ID_HEADER = "x-amzn-requestid";

export default definePlugin((nitroApp) => {
  nitroApp.hooks.hook("request", (event) => {
    const headerId = event.req.headers.get(REQUEST_ID_HEADER);
    const requestId = headerId ?? randomUUID();
    withRequest(
      { headers: { [REQUEST_ID_HEADER]: requestId } } as never,
      { awsRequestId: requestId } as never,
    );
  });
});
