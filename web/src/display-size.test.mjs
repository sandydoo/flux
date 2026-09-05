import assert from "node:assert/strict";
import test from "node:test";
import { observeDisplaySize } from "./display-size.mjs";

test("coalesces size and DPR changes before drawing, without repeated resize", () => {
  let notify;
  const canvas = { clientWidth: 1200, clientHeight: 800 };
  const host = { devicePixelRatio: 1 };
  class Observer {
    constructor(callback) { notify = callback; }
    observe(target) { assert.equal(target, canvas); }
    disconnect() {}
  }
  const sizing = observeDisplaySize(canvas, host, Observer);
  const calls = [];
  const flux = { resize: (...args) => calls.push(args) };
  sizing.apply(flux);
  sizing.apply(flux);
  assert.deepEqual(calls, [[1200, 800, 1]]);

  // Monitor transition with no ResizeObserver notification.
  host.devicePixelRatio = 2;
  sizing.apply(flux);
  sizing.apply(flux);
  assert.deepEqual(calls.at(-1), [1200, 800, 2]);
  assert.equal(calls.length, 2);

  // Page zoom and multiple layout observations coalesce into one frame.
  notify([{ contentRect: { width: 1000, height: 600 } }]);
  notify([{ contentRect: { width: 960, height: 640 } }]);
  host.devicePixelRatio = 2.5;
  assert.equal(calls.length, 2);
  sizing.apply(flux);
  assert.deepEqual(calls.at(-1), [960, 640, 2.5]);
  assert.equal(calls.length, 3);

  // A hidden canvas suspends once, then resumes even at its previous size.
  notify([{ contentRect: { width: 0, height: 0 } }]);
  sizing.apply(flux);
  sizing.apply(flux);
  assert.deepEqual(calls.at(-1), [0, 0, 2.5]);
  assert.equal(calls.length, 4);
  notify([{ contentRect: { width: 960, height: 640 } }]);
  sizing.apply(flux);
  assert.equal(calls.length, 5);
  assert.deepEqual(calls.at(-1), [960, 640, 2.5]);
});
