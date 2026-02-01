import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

interface CloudFrontRequest {
  headers: { host?: { value: string } };
  uri: string;
}

interface CloudFrontEvent {
  request: CloudFrontRequest;
}

type CloudFrontResponse =
  | CloudFrontRequest
  | {
      statusCode: number;
      statusDescription: string;
      headers: { location: { value: string } };
    };

const source = readFileSync(
  resolve(__dirname, "../../../src/cloudfront/viewer_request.js"),
  "utf8",
);

const handler = new Function(`${source}\nreturn handler;`)() as (
  event: CloudFrontEvent,
) => CloudFrontResponse;

const eventFor = (uri: string, host = "tokenoverflow.io"): CloudFrontEvent => ({
  request: { headers: { host: { value: host } }, uri },
});

describe("viewer_request", () => {
  describe("www -> apex redirect", () => {
    it("returns 301 to apex with path preserved", () => {
      const result = handler(eventFor("/blog/post", "www.tokenoverflow.io"));
      expect(result).toEqual({
        statusCode: 301,
        statusDescription: "Moved Permanently",
        headers: {
          location: { value: "https://tokenoverflow.io/blog/post" },
        },
      });
    });

    it("preserves root path", () => {
      const result = handler(eventFor("/", "www.tokenoverflow.io")) as {
        headers: { location: { value: string } };
      };
      expect(result.headers.location.value).toBe("https://tokenoverflow.io/");
    });
  });

  describe("URL rewrite", () => {
    it("appends /index.html to extensionless paths", () => {
      const result = handler(eventFor("/about")) as CloudFrontRequest;
      expect(result.uri).toBe("/about/index.html");
    });

    it("appends index.html to trailing-slash paths", () => {
      const result = handler(eventFor("/blog/")) as CloudFrontRequest;
      expect(result.uri).toBe("/blog/index.html");
    });

    it("leaves paths with extensions unchanged", () => {
      const result = handler(eventFor("/_astro/main.D8x3a9.css")) as CloudFrontRequest;
      expect(result.uri).toBe("/_astro/main.D8x3a9.css");
    });

    it("leaves /index.html unchanged", () => {
      const result = handler(eventFor("/index.html")) as CloudFrontRequest;
      expect(result.uri).toBe("/index.html");
    });

    it("leaves /favicon.ico unchanged", () => {
      const result = handler(eventFor("/favicon.ico")) as CloudFrontRequest;
      expect(result.uri).toBe("/favicon.ico");
    });
  });

  describe("missing host header", () => {
    it("falls through to URL rewrite when host header is absent", () => {
      const result = handler({
        request: { headers: {}, uri: "/about" },
      }) as CloudFrontRequest;
      expect(result.uri).toBe("/about/index.html");
    });
  });
});
