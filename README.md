![A screenshot of the fluid simulation in action](./media/header.jpg)

# Flux

> An ode to the macOS Drift screensaver that runs in the browser.

I’ve been enamoured of the Drift screensaver ever since it came out with macOS Catalina. It’s mesmerizing. I feel like it’s become an instant classic, and, dare I say, it might stand to dethrone the venerable Flurry screensaver. Hats off to the folk at Apple responsible for this gem 🙌.

This is an attempt at capturing that magic and bottling it up in a more portable vessel. This isn’t a port though. The source code for the original is locked up in a spaceship somewhere in Cupertino. Instead, consider this a delicate blend of detective work and artistic liberty. It’s WebGL2 for now, but [WebGPU](https://github.com/gfx-rs/wgpu) is shaping up nicely, so native ports aren’t off the books.


- [Prerequisites](#prerequisites)
- [Setup](#setup)
- [Develop](#develop)
- [License](#license)


## Prerequisites

- rustc with `wasm32-unknown-unknown` as a target
- cargo
- node
- pnpm


## Installation

These instructions cover macOS. Your mileage may vary.

```sh
rustup toolchain install stable
rustup target wasm32-unknown-unknown

pnpm install
```


## Develop

```sh
pnpm serve
```


## License

[MIT][license-url] © [Sander Melnikov][maintainer-url].


[license-url]: https://github.com/sandydoo/flux/blob/main/LICENSE
[maintainer-url]: https://github.com/sandydoo/
