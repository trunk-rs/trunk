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
        const msg = JSON.parse(ev.data);
        if (msg.status == "started") {
            console.log("[TRUNK]: Build started.");
        } else if (msg.status == "succeeded") {
            window.location.reload();
        } else if (msg.status == "failed") {
            console.error("[TRUNK]: Build failed.");
        } else {
            console.error("[TRUNK]: Internal error, unknown status", status);
        }
    };
    ws.onclose = reload_upon_connect;
})()
