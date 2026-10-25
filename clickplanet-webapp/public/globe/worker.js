// Globe worker: loads the globe WebAssembly bundle (built from clickplanet-globe by
// build-web.sh into this directory) and hands it the messages received while it was loading.
// The Dioxus interface talks to it with the clickplanet-globe-protocol messages.
const queued = [];
self.onmessage = (event) => queued.push(event);

const { default: init, run } = await import("./clickplanet_globe.js");
await init();
run(self, queued, undefined);
