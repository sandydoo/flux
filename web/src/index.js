import  { Flux } from '../flux-wasm';
import { Elm } from './Main.elm';


// Settings object shared between WASM and Elm
//
// It would’ve been nice to have Elm be the source of truth, but that means
// having two ports and setting up a chain of callbacks when starting
// everything up. Messy.
let settings = {
  viscosity: 0.4,
  velocityDissipation: 0.0,
  adjustAdvection: 5.0,
  fluidWidth: 128,
  fluidHeight: 128,
  diffusionIterations: 5,
  pressureIterations: 30,

  lineLength: 200.0,
  lineWidth: 8.0,
  lineBeginOffset: 0.4,

  noiseChannel1: {
    scale: 1.2,
    multiplier: 1.8,
    offset1: 1.0,
    offset2: 10.0,
    offsetIncrement: 10.0,
    blendDuration: 10.0,
  },
  noiseChannel2: {
    scale: 20.0,
    multiplier: 0.4,
    offset1: 1.0,
    offset2: 1.0,
    offsetIncrement: 0.1,
    blendDuration: 0.6,
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

