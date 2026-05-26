const chromeFlags = process.env.CI ? "--no-sandbox" : "";

module.exports = {
  ci: {
    collect: {
      staticDistDir: "apps/landing/dist",
      url: ["http://localhost:4321/"],
      numberOfRuns: 1,
      settings: {
        chromeFlags,
        formFactor: "desktop",
        screenEmulation: {
          mobile: false,
          width: 1_350,
          height: 940,
          deviceScaleFactor: 1,
          disabled: false,
        },
        throttlingMethod: "provided",
        onlyCategories: ["performance", "accessibility", "best-practices", "seo"],
      },
    },
    assert: {
      assertions: {
        "interaction-to-next-paint": "off",
        "interaction-to-next-paint-insight": "off",
        "first-meaningful-paint": "off",
        "categories:performance": ["error", { minScore: 1 }],
        "categories:accessibility": ["error", { minScore: 1 }],
        "categories:best-practices": ["error", { minScore: 1 }],
        "categories:seo": ["error", { minScore: 1 }],
        "largest-contentful-paint": ["error", { maxNumericValue: 500 }],
        "cumulative-layout-shift": ["error", { maxNumericValue: 0.05 }],
        "total-byte-weight": ["error", { maxNumericValue: 100_000 }],
      },
    },
  },
};
