const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const webpack = require('webpack');
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin');
const CopyPlugin = require('copy-webpack-plugin');

module.exports = (env, argv) => {
  const skipWasmPack = env['skip-wasm-pack'] ?? false;
  console.log(env);

  let config = {
    entry: './src/index.js',

    output: {
      path: path.resolve(__dirname, 'dist'),
      filename: 'index.js',
    },

    module: {
      rules: [{
        test: /\.elm$/,
        exclude: [/elm-stuff/],
        use: 'elm-webpack-loader',
      }],
    },

    plugins: [
      new HtmlWebpackPlugin({
        template: 'src/index.html',
      }),

      new CopyPlugin({
        patterns: [
          { from: 'public' },
        ],
      }),
    ],

    mode: 'development',

    experiments: {
      asyncWebAssembly: true,
    },
  };

  if (!skipWasmPack) {
    config.plugins.push(
      new WasmPackPlugin({
        crateDirectory: path.resolve(__dirname, '../flux-wasm'),
        watchDirectories: [
          path.resolve(__dirname, '../flux')
        ],
        outDir: path.join(__dirname, 'flux'),
      }),
    );
  }

  return config;
};
