const config = {
  plugins: {
    // Inline the app/styles/* partials into globals.css before Tailwind runs.
    // Tailwind expands each file on its own and appends variant utilities
    // (hover:, md:, ...) at the end of the file that holds `@tailwind
    // utilities`; inlining keeps them after every partial, as they were when
    // globals.css was one file. tokens.css stays a separate module.
    "postcss-import": { filter: (path) => path.startsWith("./styles/") },
    tailwindcss: {},
    autoprefixer: {},
  },
};

export default config;
