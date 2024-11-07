async function __trunkInitializer(init, source, sourceSize, initializer, initWithObject) {
  if (initializer === undefined) {
    return await init(initWithObject ? { module_or_path: source } : source);
  }

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

        const total = sourceSize;
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

  return init(initWithObject ? { module_or_path: response } : response)
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
