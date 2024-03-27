export default function myInitializer () {
  return {
    onStart: () => {
      console.log("Loading...");
      console.time("trunk-initializer");
    },
    onProgress: ({current, total}) => {
      if (!total) {
        console.log("Loading...", current, "bytes");
      } else {
        console.log("Loading...", Math.round((current/total) * 100), "%" )
      }
    },
    onComplete: () => {
      console.log("Loading... done!");
      console.timeEnd("trunk-initializer");
    },
    onSuccess: (wasm) => {
      console.log("Loading... successful!");
      console.log("WebAssembly: ", wasm);
    },
    onFailure: (error) => {
      console.warn("Loading... failed!", error);
    }
  }
};
