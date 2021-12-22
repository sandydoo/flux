import  { Flux } from '../flux-wasm';
import { Elm } from './Main.elm';


// Settings object shared between WASM and Elm
//
// It would’ve been nice to have Elm be the source of truth, but that means
// having two ports and setting up a chain of callbacks when starting
// everything up. Messy.
let settings = {
  viscosity: 1.2,
  velocityDissipation: 0.2,
  diffusionIterations: 10,
  pressureIterations: 30,
};

// Set up Elm UI
const ui = Elm.Main.init({
  node: document.getElementById('controls'),
  flags: settings,
});

// Update shared settings
ui.ports.setSettings.subscribe(function (newSettings) {
  Object.assign(settings, newSettings);
  flux.settings = settings;
});


// Set up WASM
const flux = new Flux(settings);

function animate(timestamp) {
  flux.animate(timestamp);
  window.requestAnimationFrame(animate);
}

// Start drawing
window.requestAnimationFrame(animate);

