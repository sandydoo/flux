import  { Flux } from "../flux";
import { Elm } from "./Main.elm";

let flux;

function setupFlux() {
  // Set up Elm UI
  const ui = Elm.Main.init({
    node: document.getElementById("controls"),
  });

  // Initialize WASM and run animation
  ui.ports.initFlux.subscribe(function (settings) {
    flux = new Flux(settings);

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
  ui.ports.setSettings.subscribe(function (newSettings) {
    flux.settings = newSettings;
  });
}

async function chooseImage(input) {
  const pickerOpts = {
      types: [
        {
          description: 'Images',
          accept: {
            'image/*': ['.png', '.gif', '.jpeg', '.jpg']
          }
        },
      ],
      excludeAcceptAllOption: true,
      multiple: false
    };
    let file = input.files[0];
    let image = new Image();
    image.src = URL.createObjectURL(file);
    await image.decode();
    let width = image.width;
    let height = image.height;
    console.log(width, height, file.size);
    let buffer = new Uint8Array(await file.arrayBuffer());
    console.log(buffer.length);
    flux.sample_colors_from_image(buffer);
}
window.chooseImage = chooseImage;
window.addEventListener("DOMContentLoaded", setupFlux());
