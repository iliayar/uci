/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: "class",
  content: ["./src/**/*.rs", "./index.html"],
  theme: {
    extend: {
      colors: ({ colors }) => ({
        fg: {
          light: colors.black,
          dark: colors.white,
        },
        fg2: {
          light: colors.gray[500],
          dark: colors.slate[500],
        },
        bg: {
          light: colors.gray[100],
          dark: colors.slate[800],
        },
        border: {
          light: colors.gray[200],
          dark: colors.slate[700],
        },
        "button-focus": {
          light: colors.gray[300],
          dark: colors.slate[600],
        },
        "section-active": {
          light: colors.blue[500],
          dark: colors.blue[400],
        },
        "op-in-progress": {
          light: colors.cyan[500],
          dark: colors.cyan[500],
        },
        "op-success": {
          light: colors.green[500],
          dark: colors.green[500],
        },
        "op-error": {
          light: colors.red[500],
          dark: colors.red[500],
        },
        "op-warning": {
          light: colors.yellow[500],
          dark: colors.yellow[500],
        },
        "run-button": {
          light: colors.green[500],
          dark: colors.green[500],
        },

        white: {
          0: {
            light: colors.gray[100],
            dark: colors.slate[800],
          },
          1: {
            light: colors.gray[200],
            dark: colors.slate[700],
          },
        },
        black: {
          0: {
            light: colors.black,
            dark: colors.white,
          },
          1: {
            light: colors.gray[500],
            dark: colors.slate[500],
          },
        },
        red: {
          0: {
            light: colors.red[500],
            dark: colors.red[500],
          },
        },
        yellow: {
          0: {
            light: colors.yellow[500],
            dark: colors.yellow[500],
          },
        },
      }),
      spacing: {},
    },
  },
  plugins: [],
};
