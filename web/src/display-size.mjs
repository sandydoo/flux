// Resize only at the start of a frame. Reading DPR each frame also catches
// monitor moves and page zoom when the canvas's CSS dimensions do not change.
export function observeDisplaySize(canvas, host = window, Observer = ResizeObserver) {
  let width = canvas.clientWidth;
  let height = canvas.clientHeight;
  let previous;
  const observer = new Observer(([entry]) => {
    width = entry.contentRect.width;
    height = entry.contentRect.height;
  });
  observer.observe(canvas);

  return {
    apply(flux) {
      const ratio = host.devicePixelRatio;
      const next = [Math.max(0, Math.round(width)), Math.max(0, Math.round(height)),
        Number.isFinite(ratio) && ratio > 0 ? ratio : 1];
      if (!previous || next.some((value, index) => value !== previous[index])) {
        flux.resize(...next);
        previous = next;
      }
    },
    disconnect() { observer.disconnect(); },
  };
}
