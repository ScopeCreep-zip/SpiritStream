/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class", // ✅ Ensures Tailwind respects dark mode toggling
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {},
  },
  plugins: [],
};
