import  { Flux } from '../flux-wasm';
import { Elm } from './Main.elm';


let flux;

// Set up Elm UI
const ui = Elm.Main.init({
  node: document.getElementById('controls'),
});

// Initialize WASM and run animation
ui.ports.initFlux.subscribe(function (settings) {
  flux = new Flux(settings);

  function animate(timestamp) {
    flux.animate(timestamp);
    window.requestAnimationFrame(animate);
  }

  window.requestAnimationFrame(animate);
});

// Update settings
ui.ports.setSettings.subscribe(function (newSettings) {
  flux.settings = newSettings;
});
