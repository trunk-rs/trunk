"use strict";

(function () {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const url = protocol + '//' + '{{__TRUNK_ADDRESS__}}' + '/_trunk/ws';
    const poll_interval = 5000;
    const reload_upon_connect = () => {
        window.setTimeout(
            () => {
                // when we successfully reconnect, we'll force a
                // reload (since we presumably lost connection to
                // trunk due to it being killed, so it will have
                // rebuilt on restart)
                const ws = new WebSocket(url);
                ws.onopen = () => window.location.reload();
                ws.onclose = reload_upon_connect;
            },
            poll_interval);
    };

    const ws = new WebSocket(url);
    ws.onmessage = (ev) => {
        const msg = JSON.parse(ev.data);
        if (msg.reload) {
            window.location.reload();
        }
    };
    ws.onclose = reload_upon_connect;
})()
