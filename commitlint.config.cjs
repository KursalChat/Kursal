module.exports = {
  extends: ["@commitlint/config-conventional"],
  // ignore some bots..
  ignores: [(message) => message.includes("dependabot[bot]")],
  rules: {
    "type-enum": [
      2,
      "always",
      [
        "feat",
        "fix",
        "docs",
        "style",
        "refactor",
        "perf",
        "test",
        "build",
        "ci",
        "chore",
        "revert",
      ],
    ],
    "scope-empty": [0],
    "subject-case": [0],
    "header-max-length": [2, "always", 100],
  },
};
