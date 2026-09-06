export function setupFullscreen(doc = document) {
  async function toggle() {
    if (!doc.fullscreenEnabled) return;
    try {
      if (doc.fullscreenElement) {
        await doc.exitFullscreen();
      } else {
        await doc.documentElement.requestFullscreen();
      }
    } catch (error) {
      console.error("Failed to toggle fullscreen", error);
    }
  }

  doc.addEventListener("keydown", event => {
    if (event.key.toLowerCase() !== "f" || event.repeat || event.isComposing
      || event.ctrlKey || event.metaKey || event.altKey
      || event.target?.isContentEditable
      || event.target?.closest("input, textarea, select")) return;
    event.preventDefault();
    toggle();
  });

  return toggle;
}
