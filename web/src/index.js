import { Flux as FluxGL } from "../flux-gl";
import { Flux } from "../flux";
import { Elm } from "./Main.elm";
import { observeDisplaySize } from "./display-size.mjs";
import { setupFullscreen } from "./fullscreen.mjs";

let flux;

function setupFlux() {
  // Set up Elm UI
  const ui = Elm.Main.init({
    node: document.getElementById("controls"),
  });
  ui.ports.toggleFullscreen.subscribe(setupFullscreen());

  // Initialize WASM and run animation
  ui.ports.initFlux.subscribe(async function(settings) {
    if (navigator.gpu) {
      console.log("Backend: WebGPU");
      flux = await Flux.new(settings);
    } else {
      console.log("Backend: WebGL2");
      flux = new FluxGL(settings);
    }

    if (settings.colorMode?.ImageFile) {
      loadImage(settings.colorMode.ImageFile)
        .then(bitmap => flux.save_image(bitmap))
        .catch(error => console.error("Failed to load initial color image", error));
    }

    const displaySize = observeDisplaySize(document.getElementById("canvas"));

    function animate(timestamp) {
      displaySize.apply(flux);

      flux.animate(timestamp);
      window.requestAnimationFrame(animate);
    }

    window.requestAnimationFrame(animate);
  });

  // Update settings
  ui.ports.setSettings.subscribe(async function(newSettings) {
    if (newSettings.colorMode?.ImageFile) {
      loadImage(newSettings.colorMode.ImageFile)
        .then(bitmap => flux.save_image(bitmap));
    }

    flux.settings = newSettings;
  });
}

window.addEventListener("DOMContentLoaded", setupFlux());

async function loadImage(imageUrl) {
  const response = await fetch(imageUrl);
  const blob = await response.blob();
  return createImageBitmap(blob, { resizeWidth: 500, resizeHeight: 500 });
}
