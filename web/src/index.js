import { Elm } from "./Main.elm";

async function setupFlux() {
    let flux;

    // Set up Elm UI
    const ui = Elm.Main.init({
        node: document.getElementById("controls"),
    });

    ui.ports.initFlux.subscribe(function(settings) {
        console.log("Elm init");
        flux = new Worker(new URL('./flux.js', import.meta.url));

        let canvas = document.getElementById("canvas");
        let logical_width = canvas.clientWidth;
        let logical_height = canvas.clientHeight;
        let pixel_ratio = window.devicePixelRatio;
        let physical_width = logical_width * pixel_ratio;
        let physical_height = logical_height * pixel_ratio;
        canvas.width = physical_width;
        canvas.height = physical_height;

        let offscreen = canvas.transferControlToOffscreen();
        console.log(offscreen);
        setTimeout(() => {
            flux.postMessage({ canvas: offscreen, settings }, [offscreen]);
        }, 500);
    });

    // Update settings
    ui.ports.setSettings.subscribe(function(settings) {
        // console.log("Elm update");
        flux.postMessage({ settings });
    });
}

window.addEventListener("DOMContentLoaded", setupFlux());
