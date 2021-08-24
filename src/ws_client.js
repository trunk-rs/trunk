(function () {
    var url = 'ws://' + window.location.host + '/_trunk/ws';
    var poll_interval = 5000;
    var reload_upon_connect = () => {
        window.setTimeout(
            () => {
                // when we successfully reconnect, we'll force a
                // reload (since we presumably lost connection to
                // trunk due to it being killed, so it will have
                // rebuilt on restart)
                var ws = new WebSocket(url);
                ws.onopen = () => window.location.reload();
                ws.onclose = reload_upon_connect;
            },
            poll_interval);
    };

    var ws = new WebSocket(url);
    ws.onmessage = (ev) => {
        const data = JSON.parse(ev.data);
        if (data.status == "started") {
            console.log("[TRUNK]: Build started.");
        } else if (data.status == "succeeded") {
            window.location.reload();
        } else if (data.status == "failed") {
            console.error("[TRUNK]: Build failed.");
            console.error(data.message);
        } else {
            console.error("[TRUNK]: Internal error, malformed data", data);
        }
    };
    ws.onclose = reload_upon_connect;
})()
