import  { Flux } from '../flux-wasm';
import { Elm } from './Main.elm';


// Settings object shared between WASM and Elm
//
// It would’ve been nice to have Elm be the source of truth, but that means
// having two ports and setting up a chain of callbacks when starting
// everything up. Messy.
let settings = {
  viscosity: 0.2,
  velocityDissipation: 0.01,
  adjustAdvection: 26.0,
  fluidWidth: 128,
  fluidHeight: 128,
  diffusionIterations: 5,
  pressureIterations: 30,

  lineLength: 200.0,
  lineWidth: 8.0,
  lineBeginOffset: 0.4,

  noiseChannel1: {
    scale: 1.2,
    multiplier: 1.0,
    blendDuration: 4.0,
  },
  noiseChannel2: {
    scale: 10.0,
    multiplier: 0.8,
    blendDuration: 2.0,
  },
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
