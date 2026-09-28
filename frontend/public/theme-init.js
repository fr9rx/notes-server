// Runs before first paint (loaded synchronously in <head>; the CSP forbids
// inline scripts) so the page never flashes the wrong theme.
(function () {
  var pref = "system";
  try {
    pref = localStorage.getItem("theme") || "system";
  } catch (e) {
    /* storage blocked */
  }
  var dark = pref === "dark" || (pref === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  var root = document.documentElement;
  root.setAttribute("data-theme", dark ? "dark" : "light");
  var meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute("content", dark ? "#05070D" : "#F6F4EE");
})();
