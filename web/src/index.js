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
      flux = await Flux.new(settings);
    } else {
      console.log("Backend: WebGL2");
      flux = new FluxGL(settings);
    }

    let pendingResize;

    const resizeObserver = new ResizeObserver(([entry]) => {
      // Resizing a canvas clears its backing buffer. Defer that work to the
      // animation callback so resize and redraw happen before the next paint,
      // while coalescing multiple observations into a single resize.
      pendingResize = entry.contentRect;
    });
    resizeObserver.observe(document.getElementById("canvas"));

    function animate(timestamp) {
      if (pendingResize) {
        flux.resize(pendingResize.width, pendingResize.height);
        pendingResize = undefined;
      }

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
