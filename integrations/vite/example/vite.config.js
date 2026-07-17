import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import fhtml from "vite-plugin-fhtml";

export default defineConfig({
  plugins: [fhtml(), tailwindcss()],
});
