import  { Flux } from '../flux-wasm';
import { Elm } from './Main.elm';

// Initialize Flux and return a closure to update the settings
const sendToFlux = function (settings) {
  const flux = new Flux(settings);

  function animate(timestamp) {
    flux.animate(timestamp);
    window.requestAnimationFrame(animate);
  }

  window.requestAnimationFrame(animate);

  return (settings) => flux.settings = settings;
};

// Set up Elm UI
const ui = Elm.Main.init({
  node: document.getElementById('controls'),
});

// Update settings
ui.ports.setSettings.subscribe(function (newSettings) {
  sendToFlux(newSettings);
});

