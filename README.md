<p align="center">
  <a href="https://flux.sandydoo.me/">
    <img width="100%" src="https://github.com/sandydoo/gif-storage/blob/main/flux/social-header-2022-02-03.gif" alt="Flux" />
  </a>

  <p align="center"><b>An ode to the macOS Drift screensaver that runs in the browser.</b></p>

  <p align="center">
    <a href="https://flux.sandydoo.me/">Launch in browser</a>
    &nbsp;·&nbsp;
    <a href="https://twitter.com/sandy_doo/">Follow me on Twitter</a>
    &nbsp;·&nbsp;
    <a href="https://ko-fi.com/sandydoo/">Support my work</a>
  </p>

</p>

<br>


## Backstory

I’ve been enamoured of the Drift screensaver ever since it came out with macOS Catalina. It’s mesmerizing. I feel like it’s become an instant classic, and, dare I say, it might stand to dethrone the venerable Flurry screensaver. Hats off to the folk at Apple responsible for this gem 🙌.

This is an attempt at capturing that magic and bottling it up in a more portable vessel. This isn’t a port though. The source code for the original is locked up in a spaceship somewhere in Cupertino. Instead, consider this a delicate blend of detective work and artistic liberty.


## Build

### Using Nix

Build a new release in the `result` folder:

```sh
nix build
```

Or open a development shell with all the neccessary tools:

```sh
nix develop

cd web
yarn serve
```

### Manual build

There’s a few things you’re going to have to install.

- rustc with `wasm32-unknown-unknown` as a target
- cargo
- wasm-pack
- node
- pnpm or yarn
- elm

How you get these dependencies depends on the operating system you’re running. Here’s an example for macOS and Linux using rustup:

```sh
rustup toolchain install stable
rustup target wasm32-unknown-unknown

cd web
pnpm install
```

Run a development server from the `web` folder:
```sh
pnpm serve
```

Build a release:
```sh
pnpm build
```

## License

[MIT][license-url] © [Sander Melnikov][maintainer-url].


[license-url]: https://github.com/sandydoo/flux/blob/main/LICENSE
[maintainer-url]: https://github.com/sandydoo/
