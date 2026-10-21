// ESLint flat config for the OrquestraAI React frontend.
//
// Layers: @eslint/js recommended + typescript-eslint recommended (not
// type-aware, so lint stays fast; tsc --noEmit remains the type gate) +
// react-hooks recommended-latest. Browser globals apply to src/; the
// node --test scripts under scripts/ get Node globals instead.
import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "dist/**",
      "target/**",
      "src-tauri/**",
      "node_modules/**",
      ".freebuff/logs/**",
      ".freebuff/tmp/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    // eslint-plugin-react-hooks v7 still ships legacy string-array configs,
    // which ESLint 10 flat mode rejects. Register the plugin object
    // directly with the two canonical recommended rules.
    plugins: {
      "react-hooks": reactHooks,
    },
    languageOptions: {
      globals: {
        ...globals.browser,
      },
    },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "error",
    },
  },
  {
    files: ["scripts/**/*.mjs"],
    languageOptions: {
      globals: {
        ...globals.node,
      },
    },
  },
);
