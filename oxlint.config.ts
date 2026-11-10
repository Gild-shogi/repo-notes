import { defineConfig } from "oxlint";
import reactHooks from "eslint-plugin-react-hooks";

export default defineConfig({
  jsPlugins: [
    { name: "react-hooks-js", specifier: "eslint-plugin-react-hooks" },
  ],
  overrides: [
    {
      files: ["src/**/*.{ts,tsx}"],
      rules: Object.fromEntries(
        Object.entries(reactHooks.configs.flat.recommended.rules).map(
          ([rule, severity]) => [
            rule.replace("react-hooks/", "react-hooks-js/"),
            severity,
          ],
        ),
      ),
    },
  ],
});
