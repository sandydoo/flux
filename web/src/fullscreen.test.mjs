import assert from "node:assert/strict";
import test from "node:test";
import { setupFullscreen } from "./fullscreen.mjs";

test("button and F toggle fullscreen without intercepting typing or browser shortcuts", async () => {
  let keydown;
  let requests = 0;
  let exits = 0;
  const doc = {
    fullscreenEnabled: true,
    fullscreenElement: null,
    documentElement: {
      async requestFullscreen() { requests++; doc.fullscreenElement = this; },
    },
    async exitFullscreen() { exits++; doc.fullscreenElement = null; },
    addEventListener(type, listener) { assert.equal(type, "keydown"); keydown = listener; },
  };
  const toggle = setupFullscreen(doc);
  await toggle();
  assert.equal(doc.fullscreenElement, doc.documentElement);
  await toggle();
  assert.equal(doc.fullscreenElement, null);

  let prevented = 0;
  const event = { key: "f", preventDefault() { prevented++; } };
  keydown(event);
  assert.equal(doc.fullscreenElement, doc.documentElement);
  keydown({ ...event, key: "F" });
  assert.equal(doc.fullscreenElement, null);
  assert.equal(prevented, 2);

  for (const ignored of [
    { key: "c" }, { repeat: true }, { isComposing: true },
    { ctrlKey: true }, { metaKey: true }, { altKey: true },
    { target: { isContentEditable: true } },
    { target: { closest: () => ({}) } },
  ]) keydown({ ...event, ...ignored });
  assert.equal(prevented, 2);

  doc.fullscreenEnabled = false;
  await toggle();
  assert.equal(requests, 2);
  assert.equal(exits, 2);
});
