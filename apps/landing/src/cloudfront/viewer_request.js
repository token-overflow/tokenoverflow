// CloudFront Function (cloudfront-js-2.0). Runs on viewer-request.
//
// If `var.www_domain` in Terraform ever changes, update the literal
// below to match.
function handler(event) {
  var request = event.request;
  var host = request.headers.host && request.headers.host.value;

  // www -> apex 301
  if (host === "www.tokenoverflow.io") {
    return {
      statusCode: 301,
      statusDescription: "Moved Permanently",
      headers: {
        location: { value: "https://tokenoverflow.io" + request.uri },
      },
    };
  }

  // /foo -> /foo/index.html; /foo/ -> /foo/index.html.
  // Skip if the path already targets a file with an extension.
  var uri = request.uri;
  if (uri.endsWith("/")) {
    request.uri = uri + "index.html";
  } else if (!/\.[^/]+$/.test(uri)) {
    request.uri = uri + "/index.html";
  }
  return request;
}
