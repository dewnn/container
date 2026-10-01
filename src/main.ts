import { mount } from "svelte";
import App from "./App.svelte";
import "./styles.css";
import { applyAccent, readAccent } from "./lib/accentTheme";

const accent = readAccent();
applyAccent(accent.hue, accent.saturation, false);

mount(App, { target: document.getElementById("app")! });
