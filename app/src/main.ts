import { mount } from "svelte";
import App from "./App.svelte";
import "./styles/base.css";

export default mount(App, { target: document.getElementById("app")! });
