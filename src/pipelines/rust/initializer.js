async function __trunkInitializer(source, initializer) {
  if (initializer === undefined) {
    return await init(source);
  }

  return await __trunkInitWithProgress(source, initializer);
}

async function __trunkInitWithProgress(source, initializer) {

  const {
    onStart, onProgress, onComplete, onSuccess, onFailure
  } = initializer;

  onStart?.();

  const response = fetch(source)
      .then((response) => {
        const reader = response.body.getReader();
        const headers = response.headers;
        const status = response.status;
        const statusText = response.statusText;

        const total = +response.headers.get("Content-Length");
        let current = 0;

        const stream = new ReadableStream({
          start(controller) {
            function push() {
              reader.read().then(({done, value}) => {
                if (done) {
                  onProgress?.({current: total, total});
                  controller.close();
                  return;
                }

                current += value.byteLength;
                onProgress?.({current, total});
                controller.enqueue(value);
                push();
              });
            }

            push();
          },
        });

        return {
          stream, init: {
            headers, status, statusText
          }
        };
      })
      .then(({stream, init}) =>
          new Response(stream, init),
      );

  return init(response)
      .then((value) => {
        onComplete?.();
        onSuccess?.(value);
        return value;
      }, (reason) => {
        onComplete?.();
        onFailure?.(reason);
        return reason;
      });
}
