export default function myInitializer () {
  return {
    onStart: () => {
      console.debug("Loading...");
      console.time("trunk-initializer");
    },
    onProgress: ({current, total}) => {
      if (!total) {
        console.debug("Loading...", current, "bytes");
      } else {
        console.debug("Loading...", Math.round((current/total) * 100), "%" )
      }
    },
    onComplete: () => {
      console.debug("Loading... done!");
      console.timeEnd("trunk-initializer");
    },
    onSuccess: (wasm) => {
      console.debug("Loading... successful!");
      console.debug("WebAssembly: ", wasm);
    },
    onFailure: (error) => {
      console.warn("Loading... failed!", error);
    }
  }
};
