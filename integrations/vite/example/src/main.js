import "./style.css";
import renderCard from "./card.fhtml";
import hero from "./hero.fhtml?html";

const cards = [
  {
    title: "Templated import",
    body: "card.fhtml compiled to a render function — this HTML came from data.",
  },
  {
    title: "Edit a partial",
    body: "The badge below lives in partials/badge.fhtml — change it and every card hot-reloads.",
  },
];

document.querySelector("#app").innerHTML =
  hero + cards.map((card) => renderCard(card)).join("");
