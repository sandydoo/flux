import { Flux as FluxGL } from "../flux-gl";
import { Flux } from "../flux";
import { Elm } from "./Main.elm";

let flux;

function setupFlux() {
  // Set up Elm UI
  const ui = Elm.Main.init({
    node: document.getElementById("controls"),
  });

  // Initialize WASM and run animation
  ui.ports.initFlux.subscribe(async function(settings) {
    if (navigator.gpu) {
      console.log("Backend: WebGPU");
      flux = await new Flux(settings);
    } else {
      console.log("Backend: WebGL2");
      flux = new FluxGL(settings);
    }

    function animate(timestamp) {
      flux.animate(timestamp);
      window.requestAnimationFrame(animate);
    }

    const resizeObserver = new ResizeObserver(([entry]) => {
      let { width, height } = entry.contentRect;
      flux.resize(width, height);
    });
    resizeObserver.observe(document.getElementById("canvas"));

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
