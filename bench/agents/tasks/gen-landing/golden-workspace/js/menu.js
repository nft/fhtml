const toggle = document.getElementById("menu-toggle");
const menu = document.getElementById("mobile-menu");
toggle.addEventListener("click", () => {
  const open = menu.classList.toggle("hidden") === false;
  toggle.setAttribute("aria-expanded", String(open));
});
