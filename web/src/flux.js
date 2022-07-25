import { Flux } from "../flux";

let flux;
let nextSettings;

self.onmessage = (e) => {
    if (e.data.type) {
        console.log("Webpack OK");
        return;
    }

    if (e.data.canvas) {
        console.log("New from worker");
        flux = new Flux(e.data.canvas, e.data.settings);
        function animate(timestamp) {
            if (nextSettings) {
                flux.settings = nextSettings;
                nextSettings = null;
            }
            flux.animate(timestamp);
            self.requestAnimationFrame(animate);
        }

        self.requestAnimationFrame(animate);
    } else {
        // console.log("Update from worker");
        nextSettings = e.data.settings;
    }
};
